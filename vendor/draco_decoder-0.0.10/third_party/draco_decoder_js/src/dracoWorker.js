import { createDecoderModule } from "draco3d";
import wasmUrl from "draco3d/draco_decoder.wasm?url";

let decoderModule = null;

async function getDecoderModule() {
    if (!decoderModule) {
        decoderModule = await createDecoderModule({
            locateFile: (file) => {
                if (file.endsWith(".wasm")) return wasmUrl;
                return file;
            },
        });
    }
    return decoderModule;
}


function sizeofDataType(type) {
    switch (type) {
        case decoderModule.DT_INT8: return 1; // DT_INT8
        case decoderModule.DT_UINT8: return 1; // DT_UINT8
        case decoderModule.DT_INT16: return 2; // DT_INT16
        case decoderModule.DT_UINT16: return 2; // DT_UINT16
        case decoderModule.DT_INT32: return 4; // DT_INT32
        case decoderModule.DT_UINT32: return 4; // DT_UINT32
        case decoderModule.DT_FLOAT32: return 4; // DT_FLOAT32
        case decoderModule.DT_FLOAT64: return 8; // DT_FLOAT64
        default: throw new Error("Unknown data type: " + type);
    }
}

function align(offset, alignment) {
    return (offset + alignment - 1) & ~(alignment - 1);
}

export async function parseDracoMesh(data, bufferLength) {
    await getDecoderModule();

    const startTime = performance.now();

    const decoder = new decoderModule.Decoder();
    const buffer = new decoderModule.DecoderBuffer();
    buffer.Init(new Int8Array(data), data.length);

    const geometryType = decoder.GetEncodedGeometryType(buffer);
    if (geometryType !== decoderModule.TRIANGULAR_MESH) {
        throw new Error("Unsupported geometry type");
    }

    const mesh = new decoderModule.Mesh();
    const status = decoder.DecodeBufferToMesh(buffer, mesh);
    if (!status.ok()) {
        decoderModule.destroy(mesh);
        decoderModule.destroy(decoder);
        decoderModule.destroy(buffer);
        throw new Error("Draco decoding failed: " + status.error_msg());
    }

    const numFaces = mesh.num_faces();
    const numIndices = numFaces * 3;
    const useUint16 = numIndices <= 0xffff;

    const attrCount = mesh.num_attributes();
    const numPoints = mesh.num_points();

    const outBuffer = new ArrayBuffer(bufferLength);
    const view = new DataView(outBuffer);
    let offset = 0;

    const writeU32 = (val) => {
        view.setUint32(offset, val, true);
        offset += 4;
    };

    const writeU16 = (val) => {
        view.setUint16(offset, val, true);
        offset += 2;
    };

    const writeScalar = (val, type) => {
        switch (type) {
            case decoderModule.DT_INT8: view.setInt8(offset, val); offset += 1; break;
            case decoderModule.DT_UINT8: view.setUint8(offset, val); offset += 1; break;
            case decoderModule.DT_INT16: view.setInt16(offset, val, true); offset += 2; break;
            case decoderModule.DT_UINT16: view.setUint16(offset, val, true); offset += 2; break;
            case decoderModule.DT_INT32: view.setInt32(offset, val, true); offset += 4; break;
            case decoderModule.DT_UINT32: view.setUint32(offset, val, true); offset += 4; break;
            case decoderModule.DT_FLOAT32: view.setFloat32(offset, val, true); offset += 4; break;
            case decoderModule.DT_FLOAT64: view.setFloat64(offset, val, true); offset += 8; break;
            default: throw new Error("Unknown type");
        }
    };

    const ia = new decoderModule.DracoInt32Array();
    for (let i = 0; i < numFaces; i++) {
        decoder.GetFaceFromMesh(mesh, i, ia);
        for (let j = 0; j < 3; j++) {
            const val = ia.GetValue(j);
            if (useUint16) writeU16(val);
            else writeU32(val);
        }
    }
    decoderModule.destroy(ia);

    for (let i = 0; i < attrCount; i++) {
        const attr = decoder.GetAttribute(mesh, i);
        const type = attr.data_type();
        const dim = attr.num_components();

        const pointCount = numPoints;
        const valueCount = pointCount * dim;

        const typeMap = {
            [decoderModule.DT_FLOAT32]: { arrayType: decoderModule.DracoFloat32Array, fn: 'GetAttributeFloatForAllPoints' },
            [decoderModule.DT_INT32]: { arrayType: decoderModule.DracoInt32Array, fn: 'GetAttributeInt32ForAllPoints' },
            [decoderModule.DT_UINT32]: { arrayType: decoderModule.DracoUInt32Array, fn: 'GetAttributeUInt32ForAllPoints' },
            [decoderModule.DT_INT16]: { arrayType: decoderModule.DracoInt16Array, fn: 'GetAttributeInt16ForAllPoints' },
            [decoderModule.DT_UINT16]: { arrayType: decoderModule.DracoUInt16Array, fn: 'GetAttributeUInt16ForAllPoints' },
            [decoderModule.DT_INT8]: { arrayType: decoderModule.DracoInt8Array, fn: 'GetAttributeInt8ForAllPoints' },
            [decoderModule.DT_UINT8]: { arrayType: decoderModule.DracoUInt8Array, fn: 'GetAttributeUInt8ForAllPoints' }
        };


        if (typeMap[type]) {
            const { arrayType, fn } = typeMap[type];
            const attrArray = new arrayType();
            decoder[fn](mesh, attr, attrArray);
            for (let i = 0; i < valueCount; i++) {
                writeScalar(attrArray.GetValue(i), type);
            }
            decoderModule.destroy(attrArray);
        }

        decoderModule.destroy(attr);
    }

    decoderModule.destroy(mesh);
    decoderModule.destroy(decoder);
    decoderModule.destroy(buffer);

    const endTime = performance.now();
    console.log(`Decoding took ${(endTime - startTime).toFixed(2)} ms`);


    return new Uint8Array(outBuffer, 0, offset); // 裁剪有效部分返回
}

self.onmessage = async (e) => {
    const { id, view, bufferLength } = e.data;

    try {
        const decoded = await parseDracoMesh(view, bufferLength);
        self.postMessage({ id, success: true, decoded }, [decoded.buffer]);
    } catch (err) {
        self.postMessage({ id, success: false, error: err.message });
    }
};
