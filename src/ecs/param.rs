use std::{any::TypeId, marker::PhantomData};

use bevy::{
    ecs::system::{LocalBuilder, SystemMeta, SystemParam},
    prelude::{SystemParamBuilder, *},
};
use derive_where::derive_where;

pub trait ICantBelieveItsNotClone {
    type Butter;
    fn i_cant_believe_its_not_clone(&self) -> Self::Butter;
}

impl<C: Clone> ICantBelieveItsNotClone for &C {
    type Butter = C;
    fn i_cant_believe_its_not_clone(&self) -> Self::Butter { C::clone(self) }
}

impl<T: Clone> ICantBelieveItsNotClone for LocalBuilder<T> {
    type Butter = Self;
    fn i_cant_believe_its_not_clone(&self) -> Self::Butter { LocalBuilder(self.0.clone()) }
}

#[derive_where(Copy, Clone; IfP, ElseQ)]
pub struct OverlayBuilder<P, Q, IfP, ElseQ> {
    i: IfP,
    e: ElseQ,
    _ph: PhantomData<(P, Q)>,
}

impl<P, Q, IfP, ElseQ> OverlayBuilder<P, Q, IfP, ElseQ>
where
    P: SystemParam + 'static,
    Q: SystemParam + 'static,
    IfP: SystemParamBuilder<P>,
    ElseQ: SystemParamBuilder<Q>,
{
    pub fn new(i: IfP, e: ElseQ) -> Self {
        Self {
            i,
            e,
            _ph: PhantomData,
        }
    }
}

type ParamState<P> = <P as SystemParam>::State;

unsafe impl<P, Q, IfP, ElseQ> SystemParamBuilder<Q> for OverlayBuilder<P, Q, IfP, ElseQ>
where
    P: SystemParam + 'static,
    Q: SystemParam + 'static,
    IfP: SystemParamBuilder<P>,
    ElseQ: SystemParamBuilder<Q>,
{
    fn build(self, world: &mut World, meta: &mut SystemMeta) -> ParamState<Q> {
        let p_ty = TypeId::of::<P>();
        let q_ty = TypeId::of::<Q>();
        if p_ty == q_ty {
            let mut out: ParamState<P> = self.i.build(world, meta);
            // SAFETY: We proved above that P == Q, so this operations are valid by substitution.
            //         There are also no implicit hidden lifetime parameters in ParamState.
            unsafe { (&mut out as *mut ParamState<P> as *mut ParamState<Q>).read() }
        } else {
            self.e.build(world, meta)
        }
    }
}

pub fn overlay_matching<OParam, OBuilder, Param1, Param2, Builder1, Builder2>(
    overlay: OBuilder,
    (builder1, builder2): (Builder1, Builder2),
) -> (
    OverlayBuilder<OParam, Param1, OBuilder, Builder1>,
    OverlayBuilder<OParam, Param2, OBuilder, Builder2>,
)
where
    OParam: SystemParam + 'static,
    OBuilder: SystemParamBuilder<OParam> + ICantBelieveItsNotClone<Butter = OBuilder>,
    Param1: SystemParam + 'static,
    Param2: SystemParam + 'static,
    Builder1: SystemParamBuilder<Param1>,
    Builder2: SystemParamBuilder<Param2>,
{
    (
        OverlayBuilder::new(overlay.i_cant_believe_its_not_clone(), builder1),
        OverlayBuilder::new(overlay, builder2),
    )
}

#[cfg(test)]
mod test {
    use bevy::ecs::system::{LocalBuilder, ParamBuilder};

    use super::*;

    #[test]
    fn overlay_eq() {
        let mut world = World::new();

        let overlay = LocalBuilder(true);
        let builder = OverlayBuilder::<Local<bool>, Local<bool>, _, _>::new(
            overlay,
            ParamBuilder::of::<Local<bool>>(),
        );

        let mut sys = (builder,)
            .build_state(&mut world)
            .build_any_system(|b: Local<bool>| *b);

        let result: bool = sys.run((), &mut world);

        assert!(result);
    }

    #[test]
    fn overlay_ne() {
        let mut world = World::new();

        let overlay = LocalBuilder(1);
        let builder =
            OverlayBuilder::<Local<u32>, _, _, _>::new(overlay, ParamBuilder::of::<Local<bool>>());

        let mut sys = (builder,)
            .build_state(&mut world)
            .build_any_system(|b: Local<bool>| *b);

        let result = sys.run((), &mut world);

        assert!(!result);
    }

    #[test]
    fn overlay_2() {
        let mut world = World::new();

        let overlay = LocalBuilder(true);
        macro_rules! builder {
            () => {
                (
                    ParamBuilder::of::<Local<u32>>(),
                    ParamBuilder::of::<Local<bool>>(),
                )
            };
        }
        let overlay_builder = overlay_matching(overlay, builder!());

        let sys_fn = |u: Local<u32>, b: Local<bool>| (*u, *b);
        let mut sys = builder!().build_state(&mut world).build_any_system(sys_fn);
        let mut overlay_sys = overlay_builder
            .build_state(&mut world)
            .build_any_system(sys_fn);

        let result = sys.run((), &mut world);
        let overlay_result = overlay_sys.run((), &mut world);

        // Check that the no-overlay version had a different result first, to test the test.
        assert_eq!(result, (0, false));
        assert_eq!(overlay_result, (0, true));
    }
}
