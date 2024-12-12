use std::{any::TypeId, marker::PhantomData, mem::ManuallyDrop};

use bevy::{
    ecs::system::{LocalBuilder, SystemMeta, SystemParam},
    prelude::{SystemParamBuilder, *},
    utils::all_tuples,
};
use derive_where::derive_where;

use super::GivenBuilder;

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
impl ICantBelieveItsNotClone for GivenBuilder {
    type Butter = Self;
    fn i_cant_believe_its_not_clone(&self) -> Self::Butter { self.clone() }
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
            let mut out = ManuallyDrop::<ParamState<P>>::new(self.i.build(world, meta));
            // SAFETY: We proved above that P == Q, so this operations are valid by substitution.
            //         There are also no implicit hidden lifetime parameters in ParamState.
            unsafe { (&mut out as *mut _ as *mut ParamState<Q>).read() }
        } else {
            self.e.build(world, meta)
        }
    }
}

pub trait OverlayMatching<Param: SystemParam> {
    type Param<OParam, OBuilder>;

    fn overlay_matching<OParam, OBuilder>(self, overlay: OBuilder) -> Self::Param<OParam, OBuilder>
    where
        OParam: SystemParam + 'static,
        OBuilder: SystemParamBuilder<OParam> + ICantBelieveItsNotClone<Butter = OBuilder>;
}

macro_rules! overlay_matching {
    ($(#[$meta:meta])* $(($Param:ident, $Builder:ident, $builder:ident)),*) => {
        $(#[$meta:meta])*
        #[allow(unused)]
        impl<$($Param,)* $($Builder,)*> OverlayMatching<($($Param,)*)> for ($($Builder,)*)
        where
            $($Param: SystemParam + 'static,)*
            $($Builder: SystemParamBuilder<$Param>,)*
        {
            type Param<OParam, OBuilder> = ($(OverlayBuilder<OParam, $Param, OBuilder, $Builder>,)*);

            fn overlay_matching<OParam, OBuilder>(
                self,
                overlay: OBuilder,
            ) -> Self::Param<OParam, OBuilder>
            where
                OParam: SystemParam + 'static,
                OBuilder: SystemParamBuilder<OParam> + ICantBelieveItsNotClone<Butter = OBuilder>,
            {
                let ($($builder,)*) = self;
                ($(OverlayBuilder::new(overlay.i_cant_believe_its_not_clone(), $builder),)*)
            }
        }
    };
}
all_tuples!(overlay_matching, 0, 16, Param, Builder, builder);

#[cfg(test)]
mod test {
    use bevy::ecs::system::{LocalBuilder, ParamBuilder};

    use super::*;
    use crate::ecs::{Given, GivenBuilder};

    #[test]
    fn overlay_eq() {
        let mut world = World::new();

        let overlay = LocalBuilder(true);
        let builder = (ParamBuilder::of::<Local<bool>>(),).overlay_matching(overlay);

        let mut sys = builder
            .build_state(&mut world)
            .build_any_system(|b: Local<bool>| *b);

        let result: bool = sys.run((), &mut world);

        assert!(result);
    }

    #[test]
    fn overlay_ne() {
        let mut world = World::new();

        let overlay = LocalBuilder(1);
        let builder = (ParamBuilder::of::<Local<bool>>(),).overlay_matching(overlay);

        let mut sys = builder
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
        let overlay_builder = builder!().overlay_matching(overlay);

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

    #[test]
    fn overlay_given() {
        let mut world = World::new();


        #[derive(Component)]
        struct Target;
        #[derive(Component)]
        struct PowerLevel(f32);
        #[derive(Resource)]
        struct ThagomizerLocked(Option<Entity>);

        let target_id = world.spawn((Name::new("Target"), Target)).id();
        // .5 is representable exactly
        let high_power_id = world.spawn(PowerLevel(9000.5)).id();
        let _low_power_id = world.spawn(PowerLevel(8999.5)).id();
        world.insert_resource(ThagomizerLocked(Some(target_id)));

        let builder = (
            ParamBuilder::of::<Single<&Name, With<Target>>>(),
            ParamBuilder::of::<Given<&PowerLevel>>(),
            ParamBuilder::of::<Res<ThagomizerLocked>>(),
            ParamBuilder::of::<Commands>(),
        )
            .overlay_matching::<Given<&PowerLevel>, _>(GivenBuilder::new(high_power_id));

        fn confirm_thagomizer(
            target: Single<&Name, With<Target>>,
            power_level: Given<&PowerLevel>,
            locked: Res<ThagomizerLocked>,
            _commands: Commands,
        ) -> (Option<Entity>, String, f32) {
            (locked.0, target.to_string(), power_level.get().0)
        }
        let mut overlay_sys = builder
            .build_state(&mut world)
            .build_system(confirm_thagomizer);

        let confirmation = overlay_sys.run((), &mut world);

        assert_eq!(
            confirmation,
            (Some(target_id), "Target".to_string(), 9000.5)
        );
    }
}
