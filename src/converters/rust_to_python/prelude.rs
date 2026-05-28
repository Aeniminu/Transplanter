#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Empty,
    Grass,
    Bush,
    Tree,
    Carrot,
    Pumpkin,
    Sunflower,
    Cactus,
    Dinosaur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    Grassland,
    Soil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Hay,
    Wood,
    Carrot,
    Carrot_Seed,
    Pumpkin,
    Pumpkin_Seed,
    Power,
    Cactus,
    Cactus_Seed,
    Fertilizer,
    Water,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    Speed,
    Expand,
    Plant,
    Carrots,
    Trees,
    Pumpkins,
    Sunflowers,
    Cactus,
    Dinosaurs,
    Multi_Trade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaderboard {
    Fastest_Reset,
    Maze,
    Dinosaur,
}

#[derive(Debug, Clone)]
pub struct FarmList<T = ()> {
    _marker: PhantomData<T>,
}

impl<T> FarmList<T> {
    pub fn append(&mut self, _value: T) {}
}

impl<T> Index<i32> for FarmList<T> {
    type Output = T;

    fn index(&self, _index: i32) -> &Self::Output {
        unimplemented!("transplanter_rust prelude is only for IDE checks")
    }
}

impl<T> IndexMut<i32> for FarmList<T> {
    fn index_mut(&mut self, _index: i32) -> &mut Self::Output {
        unimplemented!("transplanter_rust prelude is only for IDE checks")
    }
}

#[derive(Debug, Clone)]
pub struct FarmSet<T = ()> {
    _marker: PhantomData<T>,
}

impl<T> IntoIterator for FarmList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        Vec::new().into_iter()
    }
}

impl<T> IntoIterator for FarmSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        Vec::new().into_iter()
    }
}

#[derive(Debug, Clone)]
pub struct FarmDict<K = (), V = ()> {
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Index<K> for FarmDict<K, V> {
    type Output = V;

    fn index(&self, _index: K) -> &Self::Output {
        unimplemented!("transplanter_rust prelude is only for IDE checks")
    }
}

impl<K, V> IndexMut<K> for FarmDict<K, V> {
    fn index_mut(&mut self, _index: K) -> &mut Self::Output {
        unimplemented!("transplanter_rust prelude is only for IDE checks")
    }
}

pub fn harvest() {}

pub fn can_harvest() -> bool {
    false
}

pub fn swap(_direction: Direction) {}

pub fn plant(_entity: Entity) {}

pub fn move_dir(_direction: Direction) {}

pub fn till() {}

pub fn trade(_item: Item) {}

pub fn trade_n(_item: Item, _count: i32) {}

pub fn get_pos_x() -> i32 {
    0
}

pub fn get_pos_y() -> i32 {
    0
}

pub fn get_world_size() -> i32 {
    0
}

pub fn get_entity_type() -> Entity {
    Entity::Empty
}

pub fn get_ground_type() -> Ground {
    Ground::Grassland
}

pub fn get_tick_count() -> i32 {
    0
}

pub fn get_time() -> f64 {
    0.0
}

pub fn get_op_count() -> i32 {
    0
}

pub fn use_item(_item: Item) {}

pub fn use_item_n(_item: Item, _count: i32) {}

pub fn get_water() -> f64 {
    0.0
}

pub fn do_a_flip() {}

pub fn print<T>(_value: T) {}

pub fn quick_print<T>(_value: T) {}

pub fn len<T>(_collection: T) -> usize {
    0
}

pub fn num_items(_item: Item) -> i32 {
    0
}

pub fn get_cost<T>(_thing: T) -> i32 {
    0
}

pub fn clear() {}

pub fn get_companion() -> (Entity, (i32, i32)) {
    (Entity::Empty, (0, 0))
}

pub fn unlock(_unlock: Unlock) {}

pub fn num_unlocked<T>(_thing: T) -> i32 {
    0
}

pub fn timed_reset() {}

pub fn measure() -> i32 {
    0
}

pub fn measure_dir(_direction: Direction) -> i32 {
    0
}

pub fn min<T: Ord>(a: T, b: T) -> T {
    std::cmp::min(a, b)
}

pub fn max<T: Ord>(a: T, b: T) -> T {
    std::cmp::max(a, b)
}

pub fn abs(number: i32) -> i32 {
    number.abs()
}

pub fn random() -> f64 {
    0.0
}

pub fn list<T>() -> FarmList<T> {
    FarmList {
        _marker: PhantomData,
    }
}

pub fn set<T>() -> FarmSet<T> {
    FarmSet {
        _marker: PhantomData,
    }
}

pub fn dict<K, V>() -> FarmDict<K, V> {
    FarmDict {
        _marker: PhantomData,
    }
}

pub fn set_execution_speed(_speed: i32) {}

pub fn set_farm_size(_size: i32) {}

pub fn leaderboard_run(_leaderboard: Leaderboard, _filename: &str, _speedup: i32) {}

pub fn simulate<TUnlocks, TItems, TGlobals>(
    _filename: &str,
    _unlocks: TUnlocks,
    _items: TItems,
    _globals: TGlobals,
    _seed: i32,
    _speedup: i32,
) {
}
