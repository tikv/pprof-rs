use super::inner;

macro_rules! convert_vec {
    ($vec:ident, $type_name:ident) => {
        $vec.into_iter()
            .map(|item| item.into())
            .collect::<Vec<inner::$type_name>>()
            .into()
    };
}

macro_rules! into_inner {
    ($type_name:ident) => {
        impl Into<inner::$type_name> for $type_name {
            fn into(self) -> inner::$type_name {
                self.inner
            }
        }
    };

    ($type_name:ident, $($rest:ident),+) => {
        into_inner!($($rest),+);
        into_inner!($type_name);
    };
}

into_inner!(Profile, ValueType, Sample, Mapping, Location, Function, Label, Line);

#[derive(PartialEq, Clone, Default)]
pub struct Profile {
    inner: inner::Profile,
}

impl Profile {
    pub fn clear_sample_type(&mut self) {
        self.inner.sample_type.clear();
    }

    pub fn set_sample_type(&mut self, v: Vec<ValueType>) {
        self.inner.sample_type = convert_vec!(v, ValueType)
    }

    pub fn clear_sample(&mut self) {
        self.inner.sample.clear();
    }

    pub fn set_sample(&mut self, v: Vec<Sample>) {
        self.inner.sample = convert_vec!(v, Sample)
    }

    pub fn clear_mapping(&mut self) {
        self.inner.mapping.clear();
    }

    pub fn set_mapping(&mut self, v: Vec<Mapping>) {
        self.inner.mapping = convert_vec!(v, Mapping)
    }

    pub fn clear_location(&mut self) {
        self.inner.location.clear();
    }

    pub fn set_location(&mut self, v: Vec<Location>) {
        self.inner.location = convert_vec!(v, Location)
    }

    pub fn clear_function(&mut self) {
        self.inner.function.clear();
    }

    pub fn set_function(&mut self, v: Vec<Function>) {
        self.inner.function = convert_vec!(v, Function)
    }

    pub fn clear_string_table(&mut self) {
        self.inner.string_table.clear();
    }

    pub fn set_string_table(&mut self, v: Vec<String>) {
        self.inner.string_table = v.into();
    }

    pub fn get_drop_frames(&self) -> i64 {
        self.inner.drop_frames
    }

    pub fn clear_drop_frames(&mut self) {
        self.inner.drop_frames = 0;
    }

    pub fn set_drop_frames(&mut self, v: i64) {
        self.inner.drop_frames = v;
    }

    pub fn get_keep_frames(&self) -> i64 {
        self.inner.keep_frames
    }

    pub fn clear_keep_frames(&mut self) {
        self.inner.keep_frames = 0;
    }

    pub fn set_keep_frames(&mut self, v: i64) {
        self.inner.keep_frames = v;
    }

    pub fn get_time_nanos(&self) -> i64 {
        self.inner.time_nanos
    }

    pub fn clear_time_nanos(&mut self) {
        self.inner.time_nanos = 0;
    }

    pub fn set_time_nanos(&mut self, v: i64) {
        self.inner.time_nanos = v;
    }

    pub fn get_duration_nanos(&self) -> i64 {
        self.inner.duration_nanos
    }

    pub fn clear_duration_nanos(&mut self) {
        self.inner.duration_nanos = 0;
    }

    pub fn set_duration_nanos(&mut self, v: i64) {
        self.inner.duration_nanos = v;
    }

    pub fn get_period(&self) -> i64 {
        self.inner.period
    }

    pub fn clear_period(&mut self) {
        self.inner.period = 0;
    }

    pub fn set_period(&mut self, v: i64) {
        self.inner.period = v;
    }

    pub fn get_comment(&self) -> &[i64] {
        &self.inner.comment
    }
    pub fn clear_comment(&mut self) {
        self.inner.comment.clear();
    }

    // Param is passed by value, moved
    pub fn set_comment(&mut self, v: Vec<i64>) {
        self.inner.comment = v;
    }

    pub fn mut_comment(&mut self) -> &mut Vec<i64> {
        &mut self.inner.comment
    }

    pub fn take_comment(&mut self) -> Vec<i64> {
        ::std::mem::replace(&mut self.inner.comment, Vec::new())
    }

    pub fn get_default_sample_type(&self) -> i64 {
        self.inner.default_sample_type
    }

    pub fn clear_default_sample_type(&mut self) {
        self.inner.default_sample_type = 0;
    }

    pub fn set_default_sample_type(&mut self, v: i64) {
        self.inner.default_sample_type = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct ValueType {
    inner: inner::ValueType,
}

impl ValueType {
    #[cfg(feature = "rust-protobuf")]
    pub fn get_type(&self) -> i64 {
        self.inner.field_type
    }

    #[cfg(feature = "rust-protobuf")]
    pub fn clear_type(&mut self) {
        self.inner.field_type = 0;
    }

    #[cfg(feature = "rust-protobuf")]
    pub fn set_type(&mut self, v: i64) {
        self.inner.field_type = v;
    }

    #[cfg(feature = "prost-protobuf")]
    pub fn get_type(&self) -> i64 {
        self.inner.r#type
    }

    #[cfg(feature = "prost-protobuf")]
    pub fn clear_type(&mut self) {
        self.inner.r#type = 0;
    }

    #[cfg(feature = "prost-protobuf")]
    pub fn set_type(&mut self, v: i64) {
        self.inner.r#type = v;
    }

    pub fn get_unit(&self) -> i64 {
        self.inner.unit
    }

    pub fn clear_unit(&mut self) {
        self.inner.unit = 0;
    }

    pub fn set_unit(&mut self, v: i64) {
        self.inner.unit = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Sample {
    inner: inner::Sample,
}

impl Sample {
    pub fn clear_location_id(&mut self) {
        self.inner.location_id.clear();
    }

    pub fn set_location_id(&mut self, v: Vec<u64>) {
        self.inner.location_id = v;
    }

    pub fn mut_location_id(&mut self) -> &mut Vec<u64> {
        &mut self.inner.location_id
    }

    pub fn take_location_id(&mut self) -> Vec<u64> {
        ::std::mem::replace(&mut self.inner.location_id, Vec::new())
    }

    pub fn get_value(&self) -> &[i64] {
        &self.inner.value
    }

    pub fn clear_value(&mut self) {
        self.inner.value.clear();
    }

    pub fn set_value(&mut self, v: Vec<i64>) {
        self.inner.value = v;
    }

    pub fn mut_value(&mut self) -> &mut Vec<i64> {
        &mut self.inner.value
    }

    pub fn take_value(&mut self) -> Vec<i64> {
        ::std::mem::replace(&mut self.inner.value, Vec::new())
    }

    pub fn clear_label(&mut self) {
        self.inner.label.clear();
    }

    pub fn set_label(&mut self, v: Vec<Label>) {
        self.inner.label = convert_vec!(v, Label)
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Label {
    inner: inner::Label,
}

impl Label {
    pub fn get_key(&self) -> i64 {
        self.inner.key
    }

    pub fn clear_key(&mut self) {
        self.inner.key = 0;
    }

    pub fn set_key(&mut self, v: i64) {
        self.inner.key = v;
    }

    pub fn get_str(&self) -> i64 {
        self.inner.str
    }

    pub fn clear_str(&mut self) {
        self.inner.str = 0;
    }

    pub fn set_str(&mut self, v: i64) {
        self.inner.str = v;
    }

    pub fn get_num(&self) -> i64 {
        self.inner.num
    }

    pub fn clear_num(&mut self) {
        self.inner.num = 0;
    }

    pub fn set_num(&mut self, v: i64) {
        self.inner.num = v;
    }

    pub fn get_num_unit(&self) -> i64 {
        self.inner.num_unit
    }

    pub fn clear_num_unit(&mut self) {
        self.inner.num_unit = 0;
    }

    pub fn set_num_unit(&mut self, v: i64) {
        self.inner.num_unit = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Mapping {
    inner: inner::Mapping,
}

impl Mapping {
    pub fn get_id(&self) -> u64 {
        self.inner.id
    }

    pub fn clear_id(&mut self) {
        self.inner.id = 0;
    }

    pub fn set_id(&mut self, v: u64) {
        self.inner.id = v;
    }

    pub fn get_memory_start(&self) -> u64 {
        self.inner.memory_start
    }

    pub fn clear_memory_start(&mut self) {
        self.inner.memory_start = 0;
    }

    pub fn set_memory_start(&mut self, v: u64) {
        self.inner.memory_start = v;
    }

    pub fn get_memory_limit(&self) -> u64 {
        self.inner.memory_limit
    }

    pub fn clear_memory_limit(&mut self) {
        self.inner.memory_limit = 0;
    }

    pub fn set_memory_limit(&mut self, v: u64) {
        self.inner.memory_limit = v;
    }

    pub fn get_file_offset(&self) -> u64 {
        self.inner.file_offset
    }

    pub fn clear_file_offset(&mut self) {
        self.inner.file_offset = 0;
    }

    pub fn set_file_offset(&mut self, v: u64) {
        self.inner.file_offset = v;
    }

    pub fn get_filename(&self) -> i64 {
        self.inner.filename
    }

    pub fn clear_filename(&mut self) {
        self.inner.filename = 0;
    }

    pub fn set_filename(&mut self, v: i64) {
        self.inner.filename = v;
    }

    pub fn get_build_id(&self) -> i64 {
        self.inner.build_id
    }

    pub fn clear_build_id(&mut self) {
        self.inner.build_id = 0;
    }

    pub fn set_build_id(&mut self, v: i64) {
        self.inner.build_id = v;
    }

    pub fn get_has_functions(&self) -> bool {
        self.inner.has_functions
    }

    pub fn clear_has_functions(&mut self) {
        self.inner.has_functions = false;
    }

    pub fn set_has_functions(&mut self, v: bool) {
        self.inner.has_functions = v;
    }

    pub fn get_has_filenames(&self) -> bool {
        self.inner.has_filenames
    }

    pub fn clear_has_filenames(&mut self) {
        self.inner.has_filenames = false;
    }

    pub fn set_has_filenames(&mut self, v: bool) {
        self.inner.has_filenames = v;
    }

    pub fn get_has_line_numbers(&self) -> bool {
        self.inner.has_line_numbers
    }

    pub fn clear_has_line_numbers(&mut self) {
        self.inner.has_line_numbers = false;
    }

    pub fn set_has_line_numbers(&mut self, v: bool) {
        self.inner.has_line_numbers = v;
    }

    pub fn get_has_inline_frames(&self) -> bool {
        self.inner.has_inline_frames
    }

    pub fn clear_has_inline_frames(&mut self) {
        self.inner.has_inline_frames = false;
    }

    pub fn set_has_inline_frames(&mut self, v: bool) {
        self.inner.has_inline_frames = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Location {
    inner: inner::Location,
}

impl Location {
    pub fn get_id(&self) -> u64 {
        self.inner.id
    }

    pub fn clear_id(&mut self) {
        self.inner.id = 0;
    }

    pub fn set_id(&mut self, v: u64) {
        self.inner.id = v;
    }

    pub fn get_mapping_id(&self) -> u64 {
        self.inner.mapping_id
    }

    pub fn clear_mapping_id(&mut self) {
        self.inner.mapping_id = 0;
    }

    pub fn set_mapping_id(&mut self, v: u64) {
        self.inner.mapping_id = v;
    }

    pub fn get_address(&self) -> u64 {
        self.inner.address
    }

    pub fn clear_address(&mut self) {
        self.inner.address = 0;
    }

    pub fn set_address(&mut self, v: u64) {
        self.inner.address = v;
    }

    pub fn clear_line(&mut self) {
        self.inner.line.clear();
    }

    pub fn set_line(&mut self, v: Vec<Line>) {
        self.inner.line = convert_vec!(v, Line)
    }

    pub fn get_is_folded(&self) -> bool {
        self.inner.is_folded
    }

    pub fn clear_is_folded(&mut self) {
        self.inner.is_folded = false;
    }

    pub fn set_is_folded(&mut self, v: bool) {
        self.inner.is_folded = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Line {
    inner: inner::Line,
}

impl Line {
    pub fn get_function_id(&self) -> u64 {
        self.inner.function_id
    }

    pub fn clear_function_id(&mut self) {
        self.inner.function_id = 0;
    }

    pub fn set_function_id(&mut self, v: u64) {
        self.inner.function_id = v;
    }

    pub fn get_line(&self) -> i64 {
        self.inner.line
    }

    pub fn clear_line(&mut self) {
        self.inner.line = 0;
    }

    pub fn set_line(&mut self, v: i64) {
        self.inner.line = v;
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct Function {
    inner: inner::Function,
}

impl Function {
    pub fn get_id(&self) -> u64 {
        self.inner.id
    }

    pub fn clear_id(&mut self) {
        self.inner.id = 0;
    }

    pub fn set_id(&mut self, v: u64) {
        self.inner.id = v;
    }

    pub fn get_name(&self) -> i64 {
        self.inner.name
    }

    pub fn clear_name(&mut self) {
        self.inner.name = 0;
    }

    pub fn set_name(&mut self, v: i64) {
        self.inner.name = v;
    }

    pub fn get_system_name(&self) -> i64 {
        self.inner.system_name
    }

    pub fn clear_system_name(&mut self) {
        self.inner.system_name = 0;
    }

    pub fn set_system_name(&mut self, v: i64) {
        self.inner.system_name = v;
    }

    pub fn get_filename(&self) -> i64 {
        self.inner.filename
    }

    pub fn clear_filename(&mut self) {
        self.inner.filename = 0;
    }

    pub fn set_filename(&mut self, v: i64) {
        self.inner.filename = v;
    }

    pub fn get_start_line(&self) -> i64 {
        self.inner.start_line
    }

    pub fn clear_start_line(&mut self) {
        self.inner.start_line = 0;
    }

    pub fn set_start_line(&mut self, v: i64) {
        self.inner.start_line = v;
    }
}
