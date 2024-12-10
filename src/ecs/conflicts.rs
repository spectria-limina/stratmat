use std::fmt::Write;

use bevy::{
    ecs::{
        component::{ComponentId, Components},
        query::{Access, AccessConflicts},
    },
    prelude::*,
};
use derive_more::derive::Display;

#[derive(Debug, Display, Copy, Clone)]
pub enum BroadAccess {
    #[display("")]
    None,
    #[display("Some")]
    Some,
    #[display("**ALL**")]
    All,
}

#[derive(Debug, Display, Copy, Clone)]
pub enum BroadAccessKind {
    #[display("Read Components")]
    ReadComponents,
    #[display("Write Components")]
    WriteComponents,
    #[display("Read Resources")]
    ReadResources,
    #[display("Write Resources")]
    WriteResources,
}

#[derive(Debug, Display, Copy, Clone)]
pub enum NarrowAccess {
    #[display("")]
    None,
    #[display("Read")]
    Read,
    #[display("**WRITE**")]
    Write,
}

#[derive(Deref, Debug, Clone)]
pub struct AccessDiags {
    name: String,
    #[deref]
    access: Access<ComponentId>,
}

impl AccessDiags {
    pub fn new(name: String, access: Access<ComponentId>) -> Self { Self { name, access } }

    pub fn broad(&self, kind: BroadAccessKind) -> BroadAccess {
        type Pred = fn(&Access<ComponentId>) -> bool;
        fn on(this: &AccessDiags, any: Pred, all: Pred) -> BroadAccess {
            if all(this) {
                BroadAccess::All
            } else if any(this) {
                BroadAccess::Some
            } else {
                BroadAccess::None
            }
        }

        match kind {
            BroadAccessKind::ReadComponents => on(
                self,
                Access::has_any_component_read,
                Access::has_read_all_components,
            ),
            BroadAccessKind::WriteComponents => on(
                self,
                Access::has_any_component_write,
                Access::has_write_all_components,
            ),
            BroadAccessKind::ReadResources => on(
                self,
                Access::has_any_resource_read,
                Access::has_read_all_resources,
            ),
            BroadAccessKind::WriteResources => on(
                self,
                Access::has_any_resource_write,
                Access::has_write_all_resources,
            ),
        }
    }

    fn narrow(&self, cid: ComponentId) -> NarrowAccess {
        if self.has_component_write(cid) || self.has_resource_write(cid) {
            NarrowAccess::Write
        } else if self.has_component_read(cid) || self.has_resource_read(cid) {
            NarrowAccess::Read
        } else {
            NarrowAccess::None
        }
    }
}

#[track_caller]
pub fn diagnose_conflicts(components: &Components, new: AccessDiags, prev: Vec<AccessDiags>) {
    use prettytable::{row, Cell, Table};

    let (broad, narrow): (Vec<AccessDiags>, Vec<AccessDiags>) = prev
        .into_iter()
        .partition(|a| a.get_conflicts(&new) == AccessConflicts::All);
    let mut msg = format!(
        "\nNested system data access conflicts between {} and still-running systems:",
        new.name
    );

    if !broad.is_empty() {
        let mut table = Table::new();
        let mut titles = row![""];
        titles.add_cell(Cell::new(&BroadAccessKind::ReadComponents.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::WriteComponents.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::ReadResources.to_string()).style_spec("c"));
        titles.add_cell(Cell::new(&BroadAccessKind::WriteResources.to_string()).style_spec("c"));
        table.set_titles(titles);
        let row = table.add_row(row![br->&format!("~~{}~~", new.name)]);
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::ReadComponents).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::WriteComponents).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::ReadResources).to_string()).style_spec("c"),
        );
        row.add_cell(
            Cell::new(&new.broad(BroadAccessKind::WriteResources).to_string()).style_spec("c"),
        );

        for a in broad {
            let row = table.add_row(row![r->&a.name]);
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::ReadComponents).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::WriteComponents).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::ReadResources).to_string()).style_spec("c"),
            );
            row.add_cell(
                Cell::new(&a.broad(BroadAccessKind::WriteResources).to_string()).style_spec("c"),
            );
        }

        let _ = write!(&mut msg, "\n\n{table}");
    }

    if !narrow.is_empty() {
        let mut table = Table::new();
        let mut bits = fixedbitset::FixedBitSet::new();
        for a in &narrow {
            let AccessConflicts::Individual(bytes) = a.get_conflicts(&new) else {
                panic!("enum variant magically changed");
            };
            bits.union_with(&bytes);
        }

        let mut titles = row![""];
        let mut row = row![rb->&format!("~~{}~~", new.name)];
        for cid in bits.ones().map(ComponentId::new) {
            let name = components.get_info(cid).map_or("+++ERROR+++", |c| c.name());
            titles.add_cell(Cell::new(name).style_spec("c"));
            row.add_cell(Cell::new(&new.narrow(cid).to_string()).style_spec("cb"));
        }
        table.set_titles(titles);
        table.add_row(row);

        for a in narrow {
            let mut row = row![r->&a.name];
            for cid in bits.ones().map(ComponentId::new) {
                row.add_cell(Cell::new(&a.narrow(cid).to_string()).style_spec("c"));
            }
            table.add_row(row);
        }

        let _ = write!(&mut msg, "\n\n{table}");
    }
    error!("{}", msg);
}
