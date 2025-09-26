#include "decoder_api.h"

#include "draco/attributes/geometry_attribute.h"
#include "draco/attributes/point_attribute.h"
#include "draco/compression/decode.h"
#include "draco/compression/mesh/mesh_decoder.h"
#include "draco/compression/point_cloud/point_cloud_decoder.h"
#include "draco/core/decoder_buffer.h"
#include "draco/mesh/mesh.h"
#include "draco/point_cloud/point_cloud.h"

static size_t sizeof_data_type(draco::DataType type) {
  switch (type) {
  case draco::DT_INT8:
  case draco::DT_UINT8:
    return 1;
  case draco::DT_INT16:
  case draco::DT_UINT16:
    return 2;
  case draco::DT_INT32:
  case draco::DT_UINT32:
  case draco::DT_FLOAT32:
    return 4;
  case draco::DT_INT64:
  case draco::DT_UINT64:
  case draco::DT_FLOAT64:
    return 8;
  default:
    return 0;
  }
}

rust::Vec<uint8_t> decode_point_cloud(rust::Slice<const uint8_t> data) {
  draco::DecoderBuffer buffer;
  buffer.Init(reinterpret_cast<const char *>(data.data()), data.size());

  draco::Decoder decoder;
  auto status_or_geometry = decoder.DecodePointCloudFromBuffer(&buffer);
  if (!status_or_geometry.ok()) {
    return {};
  }

  std::unique_ptr<draco::PointCloud> pc = std::move(status_or_geometry).value();

  const draco::PointAttribute *attr =
      pc->GetNamedAttribute(draco::GeometryAttribute::POSITION);
  if (!attr) {
    return {};
  }

  rust::Vec<uint8_t> out;
  for (draco::PointIndex i(0); i < pc->num_points(); ++i) {
    float point[3] = {0.0f};
    attr->GetValue(attr->mapped_index(i), &point[0]);
    uint8_t *ptr = reinterpret_cast<uint8_t *>(point);
    for (size_t j = 0; j < sizeof(point); ++j) {
      out.push_back(ptr[j]);
    }
  }
  return out;
}

size_t decode_mesh_direct_write(const uint8_t *data, size_t data_len,
                                uint8_t *out_ptr, size_t out_len) {
  draco::DecoderBuffer buffer;
  buffer.Init(reinterpret_cast<const char *>(data), data_len);

  draco::Decoder decoder;
  auto status_or_geometry = decoder.DecodeMeshFromBuffer(&buffer);
  if (!status_or_geometry.ok()) {
    return 0;
  }

  std::unique_ptr<draco::Mesh> mesh = std::move(status_or_geometry).value();

  uint8_t *out = out_ptr;
  const size_t out_end = reinterpret_cast<size_t>(out_ptr) + out_len;

  auto write_scalar = [&](const void *src, draco::DataType type) -> bool {
    size_t size = sizeof_data_type(type);
    if (reinterpret_cast<size_t>(out) + size > out_end)
      return false;
    memcpy(out, src, size);
    out += size;
    return true;
  };

  // Write indices
  const int num_faces = mesh->num_faces();
  const int num_indices = num_faces * 3;

  bool use_u16 =
      (num_indices <= static_cast<int>(std::numeric_limits<uint16_t>::max()));

  if (use_u16) {
    for (draco::FaceIndex i(0); i < num_faces; ++i) {
      const auto &face = mesh->face(i);
      for (int j = 0; j < 3; ++j) {
        uint16_t val = static_cast<uint16_t>(face[j].value());
        if (reinterpret_cast<size_t>(out) + sizeof(uint16_t) > out_end)
          return 0;
        *reinterpret_cast<uint16_t *>(out) = val;
        out += sizeof(uint16_t);
      }
    }
  } else {
    for (draco::FaceIndex i(0); i < num_faces; ++i) {
      const auto &face = mesh->face(i);
      for (int j = 0; j < 3; ++j) {
        uint32_t val = static_cast<uint32_t>(face[j].value());
        if (reinterpret_cast<size_t>(out) + sizeof(uint32_t) > out_end)
          return 0;
        *reinterpret_cast<uint32_t *>(out) = val;
        out += sizeof(uint32_t);
      }
    }
  }

  const int attr_count = mesh->num_attributes();

  for (int i = 0; i < attr_count; ++i) {
    const draco::PointAttribute *attr = mesh->attribute(i);

    const int num_points = mesh->num_points();
    const int dim = attr->num_components();

    for (draco::PointIndex j(0); j < num_points; ++j) {
      switch (attr->data_type()) {
      case draco::DT_INT8: {
        int8_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_INT8))
            return 0;
        break;
      }
      case draco::DT_UINT8: {
        uint8_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_UINT8))
            return 0;
        break;
      }
      case draco::DT_INT16: {
        int16_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_INT16))
            return 0;
        break;
      }
      case draco::DT_UINT16: {
        uint16_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_UINT16))
            return 0;
        break;
      }
      case draco::DT_INT32: {
        int32_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_INT32))
            return 0;
        break;
      }
      case draco::DT_UINT32: {
        uint32_t v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_UINT32))
            return 0;
        break;
      }
      case draco::DT_FLOAT32: {
        float v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_FLOAT32))
            return 0;
        break;
      }
      case draco::DT_FLOAT64: {
        double v[4] = {};
        attr->ConvertValue(attr->mapped_index(j), &v[0]);
        for (int k = 0; k < dim; ++k)
          if (!write_scalar(&v[k], draco::DT_FLOAT64))
            return 0;
        break;
      }
      default:
        return 0;
      }
    }
  }

  return static_cast<size_t>(out - out_ptr);
}

size_t debug_mesh_buffer_len(const uint8_t *data, size_t data_len) {
  draco::DecoderBuffer buffer;
  buffer.Init(reinterpret_cast<const char *>(data), data_len);

  draco::Decoder decoder;
  auto status_or_geometry = decoder.DecodeMeshFromBuffer(&buffer);
  if (!status_or_geometry.ok()) {
    return 0;
  }
  std::unique_ptr<draco::Mesh> mesh = std::move(status_or_geometry).value();

  const int num_faces = mesh->num_faces();
  const int num_indices = num_faces * 3;

  const int attr_count = mesh->num_attributes();
  const int num_points = mesh->num_points();

  size_t size = 0;

  size_t index_bytes = 0;
  if (num_indices <= static_cast<int>(std::numeric_limits<uint16_t>::max())) {
    index_bytes = num_indices * sizeof(uint16_t);
  } else {
    index_bytes = num_indices * sizeof(uint32_t);
  }
  size += index_bytes;

  for (int i = 0; i < attr_count; ++i) {
    const draco::PointAttribute *attr = mesh->attribute(i);
    const int dim = attr->num_components();
    size += sizeof_data_type(attr->data_type()) * num_points * dim;
  }

  return size;
}