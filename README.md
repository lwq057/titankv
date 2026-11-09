# FlashKV
High performance KV storage engine

Hard disk storage, lightweight, TTL、GC、LZ4 compression

---

## Node.js API：
- open(config)
- put(key, buffer, ?options)
- putMany(items, ?options)
- get(key, ?options)
- getMany(keys, ?options)
- delete(key)
- deleteMany(keys)
- ttl(key, ?options)
- ttlMany(items)
- stats()
- close()
