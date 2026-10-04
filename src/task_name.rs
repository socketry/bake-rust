// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

/// Compile-time storage for a task name generated from its defining module.
/// The macro sizes the buffer from its inputs; there is no fixed name limit.
pub struct TaskName<const CAPACITY: usize> {
    bytes: [u8; CAPACITY],
    start: usize,
    length: usize,
}

impl<const CAPACITY: usize> TaskName<CAPACITY> {
    pub const fn new(name: &str, module_path: &str, infer_crate_namespace: bool) -> Self {
        let mut result = Self {
            bytes: [0; CAPACITY],
            start: 0,
            length: 0,
        };
        let name = name.as_bytes();
        let mut index = 0;
        while index < name.len() {
            if name[index] == b':' {
                result.append(name);
                return result;
            }
            index += 1;
        }

        let path = module_path.as_bytes();
        let mut crate_end = 0;
        while crate_end < path.len() && path[crate_end] != b':' {
            crate_end += 1;
        }
        if infer_crate_namespace
            && crate_end > 5
            && matches!(path, [b'b', b'a', b'k', b'e', b'_', ..])
        {
            index = 5;
            while index < crate_end {
                result.push(if path[index] == b'_' {
                    b':'
                } else {
                    path[index]
                });
                index += 1;
            }
            result.push(b':');
        }

        let prefix_length = result.length;
        index = crate_end + 2;
        while index < path.len() {
            let byte = match path[index] {
                b'_' => b'-',
                b':' => {
                    index += 1;
                    b':'
                }
                byte => byte,
            };
            result.push(byte);
            index += 1;
        }
        if result.length > prefix_length {
            result.push(b':');
            // Skip a crate prefix already expressed by complete leading modules.
            if prefix_length > 0 && result.length >= prefix_length * 2 {
                index = 0;
                while index < prefix_length
                    && result.bytes[index] == result.bytes[prefix_length + index]
                {
                    index += 1;
                }
                if index == prefix_length {
                    result.start = prefix_length;
                }
            }
        }
        result.append(name);
        result
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.length).0.split_at(self.start).1
    }

    const fn push(&mut self, byte: u8) {
        self.bytes[self.length] = byte;
        self.length += 1;
    }

    const fn append(&mut self, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            self.push(bytes[index]);
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests;
