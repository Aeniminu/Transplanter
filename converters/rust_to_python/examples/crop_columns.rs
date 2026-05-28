use transplanter_rust::prelude::*;

fn tend_tile() {
    if can_harvest() {
        harvest();
    }

    if get_ground_type() != Ground::Soil {
        till();
    }

    plant(Entity::Carrot);
}

fn main() {
    for _x in 0..4 {
        for _y in 0..4 {
            tend_tile();
            move_dir(Direction::North);
        }

        move_dir(Direction::East);
    }
}
