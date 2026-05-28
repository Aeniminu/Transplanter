use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{display_path, format_compile_error};

pub fn discover_module_files(files: &[PathBuf]) -> Result<BTreeSet<PathBuf>, String> {
    let file_set = files.iter().cloned().collect::<BTreeSet<_>>();
    let mut module_files = BTreeSet::new();

    for input_path in files {
        let source = fs::read_to_string(input_path).map_err(|err| {
            format!(
                "エラー: `{}` を読み込めません: {err}",
                display_path(input_path)
            )
        })?;
        let modules = transplanter::external_modules(&source)
            .map_err(|err| format_compile_error(input_path, err))?;

        for module in modules {
            let module_path = module_path_for(input_path, &module);
            if !file_set.contains(&module_path) {
                return Err(format!(
                    "エラー: `{}` の `mod {module};` に対応する `{}` が見つかりません",
                    display_path(input_path),
                    display_path(&module_path)
                ));
            }
            module_files.insert(module_path);
        }
    }

    Ok(module_files)
}

fn module_path_for(input_path: &Path, module: &str) -> PathBuf {
    input_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{module}.rs"))
}
