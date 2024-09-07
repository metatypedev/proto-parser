use std::path::is_separator;

use crate::proto_path::ProtoPath;

pub fn fs_path_to_proto_path(path: &ProtoPath) -> String {
    path.to_str()
        .chars()
        .map(|c| if is_separator(c) { '/' } else { c })
        .collect()
}
