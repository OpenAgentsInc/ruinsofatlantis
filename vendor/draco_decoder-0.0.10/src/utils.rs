#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeDataType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
}

impl AttributeDataType {
    pub fn size_in_bytes(&self) -> usize {
        match self {
            AttributeDataType::Int8 | AttributeDataType::UInt8 => 1,
            AttributeDataType::Int16 | AttributeDataType::UInt16 => 2,
            AttributeDataType::Int32 | AttributeDataType::UInt32 | AttributeDataType::Float32 => 4,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct MeshAttribute {
    dim: u32,
    data_type: AttributeDataType,
    offset: u32,
    lenght: u32,
}

impl MeshAttribute {
    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn lenght(&self) -> u32 {
        self.lenght
    }

    pub fn data_type(&self) -> AttributeDataType {
        self.data_type
    }

    pub fn dim(&self) -> u32 {
        self.dim
    }
}

#[derive(Debug)]
pub struct MeshDecodeConfig {
    vertex_count: u32,
    index_count: u32,
    index_length: u32,
    attributes: Vec<MeshAttribute>,
}

impl MeshDecodeConfig {
    pub fn new(vertex_count: u32, index_count: u32) -> Self {
        let index_length = if index_count <= u16::MAX as u32 {
            index_count as usize * 2
        } else {
            index_count as usize * 4
        } as u32;

        Self {
            vertex_count,
            index_count,
            index_length,
            attributes: Vec::new(),
        }
    }

    pub fn index_length(&self) -> u32 {
        self.index_length
    }

    pub fn add_attribute(&mut self, dim: u32, data_type: AttributeDataType) {
        let offset = self.estimate_buffer_size() as u32;
        let lenght = dim * self.vertex_count * data_type.size_in_bytes() as u32;
        let attribute = MeshAttribute {
            dim,
            data_type,
            offset,
            lenght,
        };
        self.attributes.push(attribute);
    }

    pub fn get_attribute(&self, index: usize) -> Option<&MeshAttribute> {
        self.attributes.get(index)
    }

    pub fn attributes(&self) -> Vec<MeshAttribute> {
        self.attributes.clone()
    }
}

impl MeshDecodeConfig {
    pub fn estimate_buffer_size(&self) -> usize {
        let mut size = 0;

        let index_bytes = if self.index_count <= u16::MAX as u32 {
            (self.index_count as usize) * 2
        } else {
            (self.index_count as usize) * 4
        };
        size += index_bytes;

        for attr in &self.attributes {
            let attr_data_size =
                (attr.dim as usize) * (self.vertex_count as usize) * attr.data_type.size_in_bytes();
            size += attr_data_size;
        }

        size
    }
}

#[derive(Debug)]
pub enum AttributeValues {
    Int8(Vec<i8>),
    UInt8(Vec<u8>),
    Int16(Vec<i16>),
    UInt16(Vec<u16>),
    Int32(Vec<i32>),
    UInt32(Vec<u32>),
    Float32(Vec<f32>),
}
