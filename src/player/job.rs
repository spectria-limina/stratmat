use bevy::prelude::*;

#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[derive(Reflect)]
pub enum Job {
    // Tanks
    Paladin,
    Warrior,
    DarkKnight,
    Gunbreaker,

    // Pure healers
    WhiteMage,
    Astrologian,

    // Barrier healers
    Scholar,
    Sage,

    // Melee DPS
    Monk,
    Dragoon,
    Ninja,
    Samurai,
    Reaper,
    Viper,

    // Physical ranged DPS
    Bard,
    Machinist,
    Dancer,

    // Magical ranged DPS
    BlackMage,
    Summoner,
    RedMage,
    Pictomancer,
    Fisher,

    // Limited jobs
    BlueMage,
    Beastmaster,
}

impl Job {
    pub fn abbrev(self) -> &'static str {
        use Job::*;
        match self {
            Paladin => "PLD",
            Warrior => "WAR",
            DarkKnight => "DRK",
            Gunbreaker => "GNB",
            WhiteMage => "WHM",
            Astrologian => "AST",
            Scholar => "SCH",
            Sage => "SGE",
            Monk => "MNK",
            Dragoon => "DRG",
            Ninja => "NIN",
            Samurai => "SAM",
            Reaper => "RPR",
            Viper => "VPR",
            Bard => "BRD",
            Machinist => "MCH",
            Dancer => "DNC",
            BlackMage => "BLM",
            Summoner => "SMN",
            RedMage => "RDM",
            Pictomancer => "PCT",
            Fisher => "FSH",
            BlueMage => "BLU",
            Beastmaster => "BSM",
        }
    }

    pub fn icon_asset_path(self) -> &'static str {
        use Job::*;
        match self {
            Paladin => "sprites/jobs/pld.png",
            Warrior => "sprites/jobs/war.png",
            DarkKnight => "sprites/jobs/drk.png",
            Gunbreaker => "sprites/jobs/gnb.png",
            WhiteMage => "sprites/jobs/whm.png",
            Astrologian => "sprites/jobs/ast.png",
            Scholar => "sprites/jobs/sch.png",
            Sage => "sprites/jobs/sge.png",
            Monk => "sprites/jobs/mnk.png",
            Dragoon => "sprites/jobs/drg.png",
            Ninja => "sprites/jobs/nin.png",
            Samurai => "sprites/jobs/sam.png",
            Reaper => "sprites/jobs/rpr.png",
            Viper => "sprites/jobs/vpr.png",
            Bard => "sprites/jobs/brd.png",
            Machinist => "sprites/jobs/mch.png",
            Dancer => "sprites/jobs/dnc.png",
            BlackMage => "sprites/jobs/blm.png",
            Summoner => "sprites/jobs/smn.png",
            RedMage => "sprites/jobs/rdm.png",
            Pictomancer => "sprites/jobs/pct.png",
            Fisher => "sprites/jobs/fsh.png",
            BlueMage => "sprites/jobs/blu.png",
            Beastmaster => Self::none_asset_path(),
        }
    }

    pub fn none_asset_path() -> &'static str { "sprites/jobs/none.png" }
}
