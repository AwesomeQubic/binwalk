use crate::common::get_cstring;
use crate::signatures::common::{SignatureError, SignatureResult, CONFIDENCE_LOW};
use aho_corasick::AhoCorasick;

/// Human readable descriptions
pub const LINUX_BOOT_IMAGE_DESCRIPTION: &str = "Linux kernel boot image";
pub const LINUX_KERNEL_VERSION_DESCRIPTION: &str = "Linux kernel version";

/// Magic bytes for a linux boot image
pub fn linux_boot_image_magic() -> Vec<Vec<u8>> {
    return vec![b"\xb8\xc0\x07\x8e\xd8\xb8\x00\x90\x8e\xc0\xb9\x00\x01\x29\xf6\x29".to_vec()];
}

/// Kernel version string magic
pub fn linux_kernel_version_magic() -> Vec<Vec<u8>> {
    return vec![b"Linux\x20version\x20".to_vec()];
}

/// Validate a linux boot image signature
pub fn linux_boot_image_parser(
    file_data: &Vec<u8>,
    offset: usize,
) -> Result<SignatureResult, SignatureError> {
    // There should be the string "!HdrS" 514 bytes from the start of the magic signature
    const HDRS_OFFSET: usize = 514;
    const HDRS_EXPECTED_VALUE: &str = "!HdrS";

    let result = SignatureResult {
        description: LINUX_BOOT_IMAGE_DESCRIPTION.to_string(),
        offset: offset,
        size: 0,
        ..Default::default()
    };

    // Calculate start and end offset of the expected !HdrS string
    let hdrs_start: usize = offset + HDRS_OFFSET;
    let hdrs_end: usize = hdrs_start + HDRS_EXPECTED_VALUE.len();

    if let Some(hdrs_bytes) = file_data.get(hdrs_start..hdrs_end) {
        // Get the string that should equal HDRS_EXPECTED_VALUE
        if let Ok(actual_hdrs_value) = String::from_utf8(hdrs_bytes.to_vec()) {
            // Validate that the hdrs string matches
            if actual_hdrs_value == HDRS_EXPECTED_VALUE {
                return Ok(result);
            }
        }
    }

    return Err(SignatureError);
}

/// Validate a linux kernel version signature and detect if a symbol table is present
pub fn linux_kernel_version_parser(
    file_data: &Vec<u8>,
    offset: usize,
) -> Result<SignatureResult, SignatureError> {
    // Kernel version string format is expected to be something like:
    // "Linux version 4.9.241 (root@server2) (gcc version 10.0.1 (OpenWrt GCC 10.0.1 r12423-0493d57e04) ) #755 SMP Wed Nov 4 03:59:02 +03 2020\n"
    const PERIOD: u8 = 0x2E;
    const NEW_LINE: &str = "\n";
    const AMPERSAND: &str = "@";
    const PERIOD_OFFSET_1: usize = 15;
    const PERIOD_OFFSET_2: usize = 17;
    const MIN_FILE_SIZE: usize = 100 * 1024;
    const MIN_VERSION_STRING_LENGTH: usize = 75;
    const GCC_VERSION_STRING: &str = "gcc ";

    let mut result = SignatureResult {
        offset: offset,
        confidence: CONFIDENCE_LOW,
        ..Default::default()
    };

    let file_size = file_data.len();

    // Sanity check the size of the file; this automatically eliminates small text files that might match the magic bytes
    if file_size > MIN_FILE_SIZE {
        // Get the kernel version string
        let kernel_version_string = get_cstring(&file_data[offset..]);

        // Sanity check the length of the version string
        if kernel_version_string.len() > MIN_VERSION_STRING_LENGTH {
            // Make sure the string includes the GCC version string too
            if kernel_version_string.contains(GCC_VERSION_STRING) {
                // Make sure the string includes an ampersand
                if kernel_version_string.contains(AMPERSAND) {
                    // The kernel version string should end with a new line
                    if kernel_version_string.ends_with(NEW_LINE) {
                        // Make sure the linux kernel version has periods at the expected locations
                        if kernel_version_string.as_bytes()[PERIOD_OFFSET_1] == PERIOD
                            && kernel_version_string.as_bytes()[PERIOD_OFFSET_2] == PERIOD
                        {
                            // Try to locate a Linux kernel symbol table
                            let symtab_present = has_linux_symbol_table(file_data);

                            // If a symbol table is present, assume the entire file is a raw Linux kernel.
                            // This is necessary for vmlinux-to-elf extraction.
                            // Otherwise just report the kernel version string and decline extraction.
                            if symtab_present {
                                result.offset = 0;
                                result.size = file_data.len();
                            } else {
                                result.size = kernel_version_string.len();
                                result.extraction_declined = true;
                            }

                            // Report the result
                            result.description = format!(
                                "{}, has symbol table: {}",
                                kernel_version_string.trim(),
                                symtab_present
                            );
                            return Ok(result);
                        }
                    }
                }
            }
        }
    }

    return Err(SignatureError);
}

/// Searches the file data for a linux symbol table
fn has_linux_symbol_table(file_data: &[u8]) -> bool {
    let mut match_count: usize = 0;

    // Same magic bytes that vmlinux-to-elf searches for
    let symtab_magic = vec![b"\x000\x001\x002\x003\x004\x005\x006\x007\x008\x009\x00"];

    let grep = AhoCorasick::new(symtab_magic).unwrap();

    // Grep for matches on the Linux symbol table magic bytes
    for _ in grep.find_overlapping_iter(file_data) {
        match_count += 1;
    }

    // There should be only one match
    return match_count == 1;
}
