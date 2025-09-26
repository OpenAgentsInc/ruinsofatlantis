import createWorker from './dracoWorker.js?worker&inline';

const worker = createWorker();

let requestId = 0;
const callbacks = new Map();

export function decodeDracoMeshInWorker(view, bufferLength) {
    return new Promise((resolve, reject) => {
        const id = requestId++;
        callbacks.set(id, { resolve, reject });

        worker.postMessage({ id, view, bufferLength }, [view.buffer]);
    });
}

worker.onmessage = (e) => {
    const { id, success, decoded, error } = e.data;
    const cb = callbacks.get(id);
    if (!cb) return;

    if (success) cb.resolve(decoded);
    else cb.reject(error);

    callbacks.delete(id);
};
