//! Pickups: health, armor, money, weapon/ammo, shops.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::player::{Player, Weapon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Health,
    Armor,
    Money,
    Weapon,
}

pub struct Pickup {
    pub pos: Vector3,
    pub kind: PickupKind,
    pub amount: f32,
    pub weapon: Option<Weapon>,
    pub active: bool,
    pub respawn_timer: f32,
}

impl Pickup {
    pub fn health(pos: Vector3) -> Self {
        Pickup { pos, kind: PickupKind::Health, amount: 50.0, weapon: None, active: true, respawn_timer: 0.0 }
    }
    pub fn armor(pos: Vector3) -> Self {
        Pickup { pos, kind: PickupKind::Armor, amount: 50.0, weapon: None, active: true, respawn_timer: 0.0 }
    }
    pub fn money(pos: Vector3, amount: i64) -> Self {
        Pickup { pos, kind: PickupKind::Money, amount: amount as f32, weapon: None, active: true, respawn_timer: 0.0 }
    }
    pub fn weapon(pos: Vector3, weapon: Weapon) -> Self {
        Pickup { pos, kind: PickupKind::Weapon, amount: 0.0, weapon: Some(weapon), active: true, respawn_timer: 0.0 }
    }

    pub fn color(&self) -> raylib::color::Color {
        match self.kind {
            PickupKind::Health => raylib::color::Color::new(220, 40, 40, 255),
            PickupKind::Armor => raylib::color::Color::new(40, 80, 220, 255),
            PickupKind::Money => raylib::color::Color::new(40, 200, 60, 255),
            PickupKind::Weapon => raylib::color::Color::new(220, 160, 40, 255),
        }
    }

    /// Try to collect. Returns true if collected.
    pub fn try_collect(&mut self, player: &mut Player) -> bool {
        if !self.active {
            return false;
        }
        if vdist_xz(self.pos, player.pos) > 2.0 {
            return false;
        }
        match self.kind {
            PickupKind::Health => player.heal(self.amount),
            PickupKind::Armor => player.add_armor(self.amount),
            PickupKind::Money => player.money += self.amount as i64,
            PickupKind::Weapon => {
                if let Some(w) = self.weapon {
                    player.weapon = w;
                    player.ammo = w.mag_size();
                    player.reserve = w.mag_size() * 4;
                }
            }
        }
        self.active = false;
        self.respawn_timer = 30.0;
        true
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            self.respawn_timer -= dt;
            if self.respawn_timer <= 0.0 && self.kind != PickupKind::Weapon && self.kind != PickupKind::Money {
                self.active = true;
            }
        }
    }
}

/// Shop: a fixed location where the player can buy things.
pub struct Shop {
    pub pos: Vector3,
    pub kind: ShopKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopKind {
    Weapon,
    Health,
}

impl Shop {
    pub fn new(pos: Vector3, kind: ShopKind) -> Self {
        Shop { pos, kind }
    }

    /// Try to buy. Returns a message string.
    pub fn try_buy(&self, player: &mut Player) -> Option<&'static str> {
        if vdist_xz(self.pos, player.pos) > 3.0 {
            return None;
        }
        match self.kind {
            ShopKind::Weapon => {
                if player.money >= 300 {
                    player.money -= 300;
                    player.weapon = Weapon::Smg;
                    player.ammo = Weapon::Smg.mag_size();
                    player.reserve = Weapon::Smg.mag_size() * 4;
                    return Some("Bought SMG!");
                }
                Some("Need $300 for SMG")
            }
            ShopKind::Health => {
                if player.money >= 100 {
                    player.money -= 100;
                    player.heal(100.0);
                    player.add_armor(50.0);
                    return Some("Healed + Armored!");
                }
                Some("Need $100 for health+armor")
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self.kind {
            ShopKind::Weapon => "Weapon Shop ($300)",
            ShopKind::Health => "Health Shop ($100)",
        }
    }
}
