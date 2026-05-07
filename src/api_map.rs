pub fn map_namespace(namespace: &str, name: &str) -> String {
    match namespace {
        "Entity" => format!("Entities.{name}"),
        "Ground" => format!("Grounds.{name}"),
        "Item" => format!("Items.{name}"),
        "Direction" if is_direction(name) => name.to_string(),
        _ => format!("{namespace}.{name}"),
    }
}

pub fn map_identifier(name: &str) -> String {
    match name {
        "true" => "True".to_string(),
        "false" => "False".to_string(),
        "move_dir" => "move".to_string(),
        _ => name.to_string(),
    }
}

fn is_direction(name: &str) -> bool {
    matches!(name, "North" | "East" | "South" | "West")
}
