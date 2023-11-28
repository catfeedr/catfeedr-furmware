# catfeedr

rustup target add thumbv6m-none-eabi

# WIP API

| Method | API | Description |
|--------|------|-------------|
| POST   | `/api/authorise/<id>` | Register `<id>` to authorised list |  
| POST   | `/api/clear/<id>` | Remove `<id>` from authorised list |  
| POST   | `/api/clear` | Remove all IDs from authorised list |  
| POST   | `/api/open` | Debug command used for opening hatch |  
| POST   | `/api/close` | Debug command used for closing hatch |  
| GET    | `/api/authorise` | Respond with all authorised `id` types |  
| GET    | `/api/status` | Respond with `status`. Subscribe to this. |  

Data formats:
| Data type | Valid values |
| --------- | ------------ |
| `status`  | Comma separated list: `lid_state`, `last_lid_open`, `last_id_auth`, `uptime`|
| `lid_state` | `open` or `closed` |
| `last_lid_open` | UNIX timestamp |
| `last_id_auth` | `id` |
| `uptime` | UNIX timestamp | 
| `id` | FDX-B ID, follows the regular expresssion r'\d{2}[A-F]\d{2}[A-F]\d{2}[A-F]\d{2}[A-F]\d{2}' (ID flipped) |