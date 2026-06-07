use zed_extension_api as zed;

struct DbtExtension;

impl zed::Extension for DbtExtension {
    fn new() -> Self {
        DbtExtension
    }
}

zed::register_extension!(DbtExtension);
