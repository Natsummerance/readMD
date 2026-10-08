# Rust后端路由与桌面桥对照

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

注册路由100条；另有动态source_id前缀。这是后端接线索引，**不是100个可见前端功能**。主清单以UI入口为分母；这里可反查每个handler。method_literals只记录handler本体出现的req.method字面比较，空白时不能推断所有HTTP方法均支持；响应、参数校验与权限以handler源码为准。前端引用行是文本使用位置，native bridge间接调用另外列。


| 路由 | handler / 源码 | 方法字面比较 | 功能引用 | 前端源码位置 | native bridge行 | 接线类别 |
| --- | --- | --- | --- | --- | --- | --- |
| /api/ping | h_ping；[rust/readmd-kernel/src/server.rs:1929](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [2206,2602] | native桥间接使用 |
| /api/kernel/status | h_kernel_status；[rust/readmd-kernel/src/server.rs:1946](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [] | 后台/控制/基础设施或仅API，未发现前端字面引用 |
| /api/file | h_file；[rust/readmd-kernel/src/server.rs:1969](../../../rust/readmd-kernel/src/server.rs) | [] | ["F001","F008","F009","F013","F015","F018","F079","F081"] | ["assets/js/core/history.js:453","assets/js/editor/preview.js:696","assets/js/reader/render.js:139"] | [3337] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/list | h_list；[rust/readmd-kernel/src/server.rs:2046](../../../rust/readmd-kernel/src/server.rs) | [] | ["F002","F008","F013"] | ["assets/js/reader/folder.js:27"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /raw | h_raw；[rust/readmd-kernel/src/server.rs:2127](../../../rust/readmd-kernel/src/server.rs) | [] | ["F019"] | ["assets/js/reader/render.js:2928,2932,3012,3017,3027"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/save | h_save；[rust/readmd-kernel/src/server.rs:2255](../../../rust/readmd-kernel/src/server.rs) | [] | ["F003","F009","F012","F021","F023","F026","F045","F082","F098"] | ["assets/js/editor/preview.js:597"] | [3166,3318,3322] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/upload | h_upload；[rust/readmd-kernel/src/server.rs:2394](../../../rust/readmd-kernel/src/server.rs) | [] | ["F001","F058"] | ["assets/js/features/convert.js:168"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/rename | h_rename；[rust/readmd-kernel/src/server.rs:2649](../../../rust/readmd-kernel/src/server.rs) | [] | ["F011"] | [] | [3943] | native桥间接使用 |
| /api/recent/status | h_recent_status；[rust/readmd-kernel/src/server.rs:2783](../../../rust/readmd-kernel/src/server.rs) | [] | ["F005"] | ["assets/js/core/history.js:13,61"] | [2707,2715,3383,3417] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/recent/add | h_recent_add；[rust/readmd-kernel/src/server.rs:2817](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/core/history.js:221"] | [3390] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/recent/remove | h_recent_remove；[rust/readmd-kernel/src/server.rs:2849](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/core/history.js:29"] | [3400] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/recent/clear | h_recent_clear；[rust/readmd-kernel/src/server.rs:2875](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/core/history.js:201"] | [3410] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/links/index | h_links_index；[rust/readmd-kernel/src/server.rs:2904](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F078"] | ["assets/js/features/graph.js:651"] | [3631] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/links/graph | h_links_graph；[rust/readmd-kernel/src/server.rs:2957](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/features/graph.js:647"] | [3607] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/links/backlinks | h_links_backlinks；[rust/readmd-kernel/src/server.rs:2983](../../../rust/readmd-kernel/src/server.rs) | [] | ["F080"] | ["assets/js/features/graph.js:732"] | [3625] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/links/deadlinks | h_links_deadlinks；[rust/readmd-kernel/src/server.rs:3012](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [] | 后台/控制/基础设施或仅API，未发现前端字面引用 |
| /api/settings | h_settings；[rust/readmd-kernel/src/server.rs:3050](../../../rust/readmd-kernel/src/server.rs) | ["GET"] | ["F016","F017","F027","F041"] | [] | [2723,3429,3436,3448] | native桥间接使用 |
| /api/style/get | h_style_get；[rust/readmd-kernel/src/server.rs:3134](../../../rust/readmd-kernel/src/server.rs) | [] | ["F083"] | ["assets/app.js:1331"] | [3458] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/style/save | h_style_save；[rust/readmd-kernel/src/server.rs:3169](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/app.js:1365"] | [3464] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/system/language | h_language；[rust/readmd-kernel/src/server.rs:3237](../../../rust/readmd-kernel/src/server.rs) | ["GET"] | [] | [] | [3766] | native桥间接使用 |
| /api/autostart/get | h_autostart_get；[rust/readmd-kernel/src/server.rs:3252](../../../rust/readmd-kernel/src/server.rs) | [] | ["F084"] | ["assets/js/core/settings.js:135"] | [3366] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/autostart/set | h_autostart_set；[rust/readmd-kernel/src/server.rs:3257](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/core/settings.js:160"] | [3373] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/modules | h_modules；[rust/readmd-kernel/src/server.rs:3372](../../../rust/readmd-kernel/src/server.rs) | [] | ["F041"] | ["assets/js/core/modules.js:17,78,85","assets/js/features/ai.js:176","assets/js/reader/render.js:3087,3094"] | [2516,2590,2591,2593,2595,3782] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/skills | h_skills；[rust/readmd-kernel/src/server.rs:3387](../../../rust/readmd-kernel/src/server.rs) | [] | ["F052","F053","F055"] | ["assets/js/features/ai.js:203,667,701,713,729,749"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets | parity_pets::h_pets；[rust/readmd-kernel/src/parity_pets.rs:4014](../../../rust/readmd-kernel/src/parity_pets.rs) | ["GET"] | ["F086","F087","F088","F089","F090","F091","F092"] | ["assets/js/features/pet-batch.js:55,82,86,509,941,980,993,1235,1313,1335,1349,1406,1417,1441,1586,1612","assets/js/features/pet-companion-actions.js:52","assets/js/features/pet-workbench.js:231,248,374"] | [3788,3794,3804,3814,3820,3830,3836,3842,3848,3854,3864,3874,3884,3890,3896] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/status | parity_pets::h_pets_status；[rust/readmd-kernel/src/parity_pets.rs:4027](../../../rust/readmd-kernel/src/parity_pets.rs) | ["GET"] | ["F086"] | ["assets/js/features/pet-batch.js:55"] | [3788] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/plugins/list | h_plugins_list；[rust/readmd-kernel/src/server.rs:3453](../../../rust/readmd-kernel/src/server.rs) | [] | ["F062"] | ["assets/js/features/convert.js:235"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/plugins/toggle | crate::plugin_manager::h_plugins_toggle；[rust/readmd-kernel/src/plugin_manager.rs:1237](../../../rust/readmd-kernel/src/plugin_manager.rs) | ["POST"] | ["F064"] | ["assets/js/features/convert.js:555"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ai/config | h_ai_config；[rust/readmd-kernel/src/server.rs:3466](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F041","F046","F047","F048","F049","F050"] | ["assets/js/features/ai.js:1490,1746,1807"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ai/models | h_ai_models；[rust/readmd-kernel/src/server.rs:3515](../../../rust/readmd-kernel/src/server.rs) | [] | ["F047","F050"] | ["assets/js/features/ai.js:1969,2307"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ai/chat | h_ai_chat；[rust/readmd-kernel/src/server.rs:4005](../../../rust/readmd-kernel/src/server.rs) | [] | ["F024","F042","F043","F044","F045","F054","F073","F082","F083","F098"] | ["assets/app.js:785","assets/js/editor/editor.js:1468","assets/js/features/ai.js:2152","assets/js/features/export.js:1511","assets/js/reader/fixes.js:54"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ai/history | h_ai_history；[rust/readmd-kernel/src/server.rs:4062](../../../rust/readmd-kernel/src/server.rs) | ["GET"] | ["F042","F051"] | ["assets/js/features/ai.js:1142,1189,1192,1210,1435,1453,1704,1715,2346"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ai/prompts | h_ai_prompts；[rust/readmd-kernel/src/server.rs:4091](../../../rust/readmd-kernel/src/server.rs) | ["GET"] | ["F041","F052","F053","F056"] | ["assets/js/features/ai.js:203,874,1090,1123"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/image/save | h_image_save；[rust/readmd-kernel/src/server.rs:4116](../../../rust/readmd-kernel/src/server.rs) | [] | ["F026","F028","F030"] | ["assets/js/editor/editor.js:1185","assets/js/editor/image.js:381"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/url | h_url_parity；[rust/readmd-kernel/src/server.rs:4199](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [] | 后台/控制/基础设施或仅API，未发现前端字面引用 |
| /api/web/extract | h_web_extract_parity；[rust/readmd-kernel/src/server.rs:4207](../../../rust/readmd-kernel/src/server.rs) | [] | ["F067","F068","F101"] | ["assets/js/features/web.js:55"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/web/cancel | h_web_cancel_parity；[rust/readmd-kernel/src/server.rs:4213](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/features/web.js:141"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/bibtex | h_bibtex；[rust/readmd-kernel/src/server.rs:4219](../../../rust/readmd-kernel/src/server.rs) | [] | ["F020"] | ["assets/js/reader/render.js:323"] | [3656] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/diagram/capabilities | parity_diagram::h_diagram_capabilities；[rust/readmd-kernel/src/parity_diagram.rs:33](../../../rust/readmd-kernel/src/parity_diagram.rs) | [] | [] | [] | [3542] | native桥间接使用 |
| /api/control/open | h_control_open；[rust/readmd-kernel/src/server.rs:4375](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [2223,2229,2603,2629,2633] | native桥间接使用 |
| /api/control/next | h_control_next；[rust/readmd-kernel/src/server.rs:4397](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/reader/render.js:260"] | [2636,2641] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/control/pet-batch | h_control_pet_batch；[rust/readmd-kernel/src/server.rs:4416](../../../rust/readmd-kernel/src/server.rs) | [] | ["F095"] | ["assets/js/features/pet-batch.js:1164"] | [3906,3916] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/control/pet-menu | h_control_pet_menu；[rust/readmd-kernel/src/server.rs:4416](../../../rust/readmd-kernel/src/server.rs) | [] | ["F094"] | ["assets/js/features/pet-batch.js:1165"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/import/process | h_import_process；[rust/readmd-kernel/src/server.rs:4258](../../../rust/readmd-kernel/src/server.rs) | [] | ["F035"] | ["assets/js/reader/render.js:860"] | [3567] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/modules/load | h_modules_load；[rust/readmd-kernel/src/server.rs:6546](../../../rust/readmd-kernel/src/server.rs) | [] | ["F041"] | ["assets/js/core/modules.js:78","assets/js/features/ai.js:176","assets/js/reader/render.js:3087"] | [2516] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/upstream-sources | h_upstream_sources；[rust/readmd-kernel/src/server.rs:6640](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [] | 后台/控制/基础设施或仅API，未发现前端字面引用 |
| /api/share/start | h_share_start；[rust/readmd-kernel/src/server.rs:6947](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F077"] | ["assets/js/features/share.js:118"] | [4309,4314] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/share/status | h_share_status；[rust/readmd-kernel/src/server.rs:7012](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/features/share.js:56"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/share/stop | h_share_stop；[rust/readmd-kernel/src/server.rs:7021](../../../rust/readmd-kernel/src/server.rs) | [] | [] | ["assets/js/features/share.js:118"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/active | parity_pets::h_pet_active；[rust/readmd-kernel/src/parity_pets.rs:4169](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | ["F087"] | ["assets/js/features/pet-batch.js:1417","assets/js/features/pet-workbench.js:231,248"] | [3874] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/remove | parity_pets::h_pet_remove；[rust/readmd-kernel/src/parity_pets.rs:4134](../../../rust/readmd-kernel/src/parity_pets.rs) | ["DELETE","POST"] | [] | ["assets/js/features/pet-batch.js:1441"] | [3864] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/thumb | parity_pets::h_pet_thumb；[rust/readmd-kernel/src/parity_pets.rs:4040](../../../rust/readmd-kernel/src/parity_pets.rs) | ["GET"] | [] | ["assets/js/features/pet-batch.js:509,941,980,993","assets/js/features/pet-workbench.js:374"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/update_status | parity_pets::h_pet_update_status；[rust/readmd-kernel/src/parity_pets.rs:4434](../../../rust/readmd-kernel/src/parity_pets.rs) | ["GET"] | [] | ["assets/js/features/pet-batch.js:1335"] | [3814] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/uninstall | parity_pets::h_pet_uninstall；[rust/readmd-kernel/src/parity_pets.rs:4347](../../../rust/readmd-kernel/src/parity_pets.rs) | [] | [] | ["assets/js/features/pet-batch.js:1235"] | [3842] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/interact | parity_pets::h_pet_interact；[rust/readmd-kernel/src/parity_pets.rs:4393](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | ["F091"] | ["assets/js/features/pet-companion-actions.js:52"] | [3804] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/import | parity_pets::h_pet_import；[rust/readmd-kernel/src/parity_pets.rs:4081](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | ["F088"] | ["assets/js/features/pet-batch.js:1406"] | [3854] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/export | h_export；[rust/readmd-kernel/src/server.rs:7438](../../../rust/readmd-kernel/src/server.rs) | [] | ["F070","F071","F072","F074","F076"] | ["assets/js/features/export.js:187,811,884,1277,1318,1327,1344,1452","assets/js/reader/render.js:2893"] | [3474,3484,3494,3953,3959] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/export/preview | h_export_preview；[rust/readmd-kernel/src/server.rs:7417](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F072"] | ["assets/js/features/export.js:811,884"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/task/cancel | h_task_cancel；[rust/readmd-kernel/src/server.rs:7598](../../../rust/readmd-kernel/src/server.rs) | [] | ["F074"] | ["assets/js/core/task-feedback.js:14,65"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/export/presets | h_export_presets；[rust/readmd-kernel/src/server.rs:7310](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F070","F071"] | ["assets/js/features/export.js:187,1452"] | [3953,3959] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/update/check | batch2::h_update_check；[rust/readmd-kernel/src/batch2.rs:205](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F085"] | ["assets/js/features/updater.js:23"] | [3705] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/update/download | batch2::h_update_download；[rust/readmd-kernel/src/batch2.rs:660](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/updater.js:149"] | [3734] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/update/status | batch2::h_update_status；[rust/readmd-kernel/src/batch2.rs:702](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/updater.js:183"] | [3744] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/update/cancel | batch2::h_update_cancel；[rust/readmd-kernel/src/batch2.rs:745](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/updater.js:256"] | [3750] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/update/apply | batch2::h_update_apply；[rust/readmd-kernel/src/batch2.rs:887](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/updater.js:276"] | [3756] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/diagram/render | parity_diagram::h_diagram_render；[rust/readmd-kernel/src/parity_diagram.rs:38](../../../rust/readmd-kernel/src/parity_diagram.rs) | [] | ["F039","F040"] | ["assets/js/reader/render.js:2464,2479"] | [3525] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/export/epub | batch2::h_export_epub；[rust/readmd-kernel/src/batch2.rs:1135](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F074"] | ["assets/js/features/export.js:1318"] | [3484] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/plugins/uninstall | crate::plugin_manager::h_plugins_uninstall；[rust/readmd-kernel/src/plugin_manager.rs:1311](../../../rust/readmd-kernel/src/plugin_manager.rs) | ["POST"] | ["F065"] | ["assets/js/features/convert.js:634"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/skill-imports | batch2::h_skill_imports_list；[rust/readmd-kernel/src/batch2.rs:1303](../../../rust/readmd-kernel/src/batch2.rs) | ["DELETE","GET"] | ["F057"] | ["assets/js/features/ai.js:941,1046"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/skill-imports/preview | batch2::h_skill_imports_preview；[rust/readmd-kernel/src/batch2.rs:1358](../../../rust/readmd-kernel/src/batch2.rs) | ["POST"] | ["F057"] | ["assets/js/features/ai.js:941"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/skill-imports/apply | batch2::h_skill_imports_apply；[rust/readmd-kernel/src/batch2.rs:1468](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/ai.js:1046"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/convert/collect | batch2::h_convert_collect；[rust/readmd-kernel/src/batch2.rs:1606](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F058"] | ["assets/js/features/convert.js:67"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/convert/progress | batch2::h_convert_progress；[rust/readmd-kernel/src/batch2.rs:1626](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F059"] | ["assets/js/features/batch.js:173"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/convert/cancel | batch2::h_convert_cancel；[rust/readmd-kernel/src/batch2.rs:1665](../../../rust/readmd-kernel/src/batch2.rs) | [] | [] | ["assets/js/features/batch.js:163"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/batch/extract-zip | batch2::h_batch_extract_zip；[rust/readmd-kernel/src/batch2.rs:1786](../../../rust/readmd-kernel/src/batch2.rs) | ["POST"] | ["F061"] | ["assets/js/core/dragdrop.js:26,33"] | [3585] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/install | parity_pets::h_pet_install；[rust/readmd-kernel/src/parity_pets.rs:4342](../../../rust/readmd-kernel/src/parity_pets.rs) | [] | [] | ["assets/js/features/pet-batch.js:1235"] | [3836,3884] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/check_update | parity_pets::h_pet_check_update；[rust/readmd-kernel/src/parity_pets.rs:4460](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | [] | ["assets/js/features/pet-batch.js:1313"] | [3820] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/apply_update | parity_pets::h_pet_apply_update；[rust/readmd-kernel/src/parity_pets.rs:4491](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | [] | ["assets/js/features/pet-batch.js:1349"] | [3830] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/configure | parity_pets::h_pet_configure；[rust/readmd-kernel/src/parity_pets.rs:4371](../../../rust/readmd-kernel/src/parity_pets.rs) | ["POST"] | ["F089","F092"] | ["assets/js/features/pet-batch.js:82,86"] | [3794,3896] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/pets/runtime/install | parity_pets::h_pet_runtime_install；[rust/readmd-kernel/src/parity_pets.rs:4358](../../../rust/readmd-kernel/src/parity_pets.rs) | [] | ["F090"] | ["assets/js/features/pet-batch.js:1586"] | [3890] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/plugins/install | crate::plugin_manager::h_plugins_install；[rust/readmd-kernel/src/plugin_manager.rs:1291](../../../rust/readmd-kernel/src/plugin_manager.rs) | ["POST"] | ["F063"] | ["assets/js/features/convert.js:601"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/export/presentation | batch2::h_export_presentation；[rust/readmd-kernel/src/batch2.rs:2141](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F074","F076"] | ["assets/js/features/export.js:1327","assets/js/reader/render.js:2893"] | [3474] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/code/run | parity_code::h_code_run；[rust/readmd-kernel/src/parity_code.rs:44](../../../rust/readmd-kernel/src/parity_code.rs) | [] | ["F033","F037","F038"] | ["assets/js/reader/render.js:1802"] | [3512] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/ocr | h_ocr_parity；[rust/readmd-kernel/src/server.rs:4186](../../../rust/readmd-kernel/src/server.rs) | [] | ["F059","F066"] | ["assets/js/features/batch.js:235","assets/js/features/convert.js:108"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/transcribe | h_transcribe_parity；[rust/readmd-kernel/src/server.rs:4193](../../../rust/readmd-kernel/src/server.rs) | [] | ["F100"] | [] | [3548] | native桥间接使用 |
| /api/convert | batch2::h_convert；[rust/readmd-kernel/src/batch2.rs:2658](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F058","F059","F060","F100"] | ["assets/js/features/batch.js:147,163,173","assets/js/features/convert.js:67","assets/js/reader/render.js:3116"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/convert/batch | batch2::h_convert_batch；[rust/readmd-kernel/src/batch2.rs:2737](../../../rust/readmd-kernel/src/batch2.rs) | [] | ["F100"] | ["assets/js/features/batch.js:147"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/dialog/choose-folder | h_dialog_choose_folder；[rust/readmd-kernel/src/server.rs:7075](../../../rust/readmd-kernel/src/server.rs) | [] | ["F002"] | [] | [3247,3275] | native桥间接使用 |
| /api/dialog/choose-file | h_dialog_choose_file；[rust/readmd-kernel/src/server.rs:7081](../../../rust/readmd-kernel/src/server.rs) | [] | ["F001"] | [] | [3254,3275,3283] | native桥间接使用 |
| /api/dialog/choose-any-file | h_dialog_choose_any_file；[rust/readmd-kernel/src/server.rs:7087](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [3261] | native桥间接使用 |
| /api/dialog/choose-many-files | h_dialog_choose_many_files；[rust/readmd-kernel/src/server.rs:7093](../../../rust/readmd-kernel/src/server.rs) | [] | ["F058"] | [] | [3268] | native桥间接使用 |
| /api/dialog/save-file | h_dialog_save_file；[rust/readmd-kernel/src/server.rs:7102](../../../rust/readmd-kernel/src/server.rs) | [] | ["F003"] | [] | [] | 后台/控制/基础设施或仅API，未发现前端字面引用 |
| /api/dialog/save-as | h_dialog_save_as；[rust/readmd-kernel/src/server.rs:7151](../../../rust/readmd-kernel/src/server.rs) | [] | ["F003","F012","F021","F102"] | [] | [3299] | native桥间接使用 |
| /api/file/save-fixed | h_file_save_fixed；[rust/readmd-kernel/src/server.rs:7626](../../../rust/readmd-kernel/src/server.rs) | [] | ["F081"] | [] | [3337] | native桥间接使用 |
| /api/system/open-path | h_system_open_path；[rust/readmd-kernel/src/server.rs:7122](../../../rust/readmd-kernel/src/server.rs) | [] | ["F018","F059"] | ["assets/js/features/batch.js:289"] | [3972,3990,4002,4021] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/system/reveal-path | h_system_reveal_path；[rust/readmd-kernel/src/server.rs:7143](../../../rust/readmd-kernel/src/server.rs) | [] | [] | [] | [4012] | native桥间接使用 |
| /api/system/assoc | h_system_assoc；[rust/readmd-kernel/src/server.rs:7645](../../../rust/readmd-kernel/src/server.rs) | [] | ["F084"] | [] | [3348] | native桥间接使用 |
| /api/clipboard/read | h_clipboard_read；[rust/readmd-kernel/src/server.rs:7677](../../../rust/readmd-kernel/src/server.rs) | [] | ["F067","F069"] | [] | [3358] | native桥间接使用 |
| /api/clipboard/convert-html | h_clipboard_convert_html；[rust/readmd-kernel/src/server.rs:7381](../../../rust/readmd-kernel/src/server.rs) | ["POST"] | ["F069"] | ["assets/js/features/clipboard.js:69"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |
| /api/documents/history | h_document_history；[rust/readmd-kernel/src/server.rs:2108](../../../rust/readmd-kernel/src/server.rs) | ["GET","POST"] | ["F021","F102","F103"] | ["assets/js/features/document-history.js:11"] | [] | 前端文本引用（含委托/常量/注释，需结合主流程） |

## 动态来源前缀（不是常驻可点菜单）


| 前缀 | handler | 说明 |
| --- | --- | --- |
| /api/skill-imports/{source_id} | [rust/readmd-kernel/src/server.rs:1047](../../../rust/readmd-kernel/src/server.rs) | 单来源check/update/delete等分支；当前没有完整来源管理常驻面板，不能从API推导UI入口 |
| /api/upstream-sources/{source_id} | [rust/readmd-kernel/src/server.rs:6638](../../../rust/readmd-kernel/src/server.rs) | 单来源check/update/delete等分支；当前没有完整来源管理常驻面板，不能从API推导UI入口 |

## pywebview.api名称的Rust桌面桥

这是Rust宿主注入的兼容对象，不证明Python仍在运行。每条列其内部端点与主前端引用；动态构造的URL会只显示字面部分。


| bridge方法 | 注入源码 | 内部URL | 前端引用 |
| --- | --- | --- | --- |
| choose_folder | [rust/readmd-kernel/src/main.rs:3245](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-folder"] | [{"file":"assets/js/features/convert.js","lines":[64]},{"file":"assets/js/reader/folder.js","lines":[17]}] |
| choose_file | [rust/readmd-kernel/src/main.rs:3252](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-file"] | [{"file":"assets/js/editor/editor.js","lines":[893,898]},{"file":"assets/js/reader/render.js","lines":[3164]}] |
| choose_any_file | [rust/readmd-kernel/src/main.rs:3259](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-any-file"] | [{"file":"assets/js/features/convert.js","lines":[138]}] |
| choose_many_files | [rust/readmd-kernel/src/main.rs:3266](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-many-files"] | [{"file":"assets/js/features/convert.js","lines":[42,124]}] |
| choose_skill_source | [rust/readmd-kernel/src/main.rs:3273](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-file","/api/dialog/choose-folder"] | [{"file":"assets/js/features/ai.js","lines":[919,920]}] |
| choose_pet_plugin | [rust/readmd-kernel/src/main.rs:3281](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/choose-file"] | [] |
| save_as | [rust/readmd-kernel/src/main.rs:3288](../../../rust/readmd-kernel/src/main.rs) | ["/api/dialog/save-as"] | [{"file":"assets/js/editor/preview.js","lines":[689]},{"file":"assets/js/features/ai.js","lines":[2364,2365,2704]}] |
| save_file | [rust/readmd-kernel/src/main.rs:3320](../../../rust/readmd-kernel/src/main.rs) | ["/api/save"] | [] |
| save_fixed | [rust/readmd-kernel/src/main.rs:3335](../../../rust/readmd-kernel/src/main.rs) | ["/api/file/save-fixed"] | [{"file":"assets/app.js","lines":[479]}] |
| install_association | [rust/readmd-kernel/src/main.rs:3346](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/assoc"] | [{"file":"assets/js/core/history.js","lines":[517]},{"file":"assets/js/core/state.js","lines":[185]}] |
| authorize_clipboard_read | [rust/readmd-kernel/src/main.rs:3353](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/features/clipboard.js","lines":[11,12]}] |
| read_clipboard | [rust/readmd-kernel/src/main.rs:3356](../../../rust/readmd-kernel/src/main.rs) | ["/api/clipboard/read"] | [{"file":"assets/app.js","lines":[301,302]},{"file":"assets/js/editor/editor.js","lines":[337,338]},{"file":"assets/js/features/clipboard.js","lines":[14,16]}] |
| get_autostart | [rust/readmd-kernel/src/main.rs:3364](../../../rust/readmd-kernel/src/main.rs) | ["/api/autostart/get"] | [{"file":"assets/js/core/settings.js","lines":[132,133]}] |
| set_autostart | [rust/readmd-kernel/src/main.rs:3371](../../../rust/readmd-kernel/src/main.rs) | ["/api/autostart/set"] | [{"file":"assets/js/core/settings.js","lines":[157,158]}] |
| get_recent | [rust/readmd-kernel/src/main.rs:3381](../../../rust/readmd-kernel/src/main.rs) | ["/api/recent/status"] | [{"file":"assets/js/core/history.js","lines":[9,10]}] |
| add_recent | [rust/readmd-kernel/src/main.rs:3388](../../../rust/readmd-kernel/src/main.rs) | ["/api/recent/add"] | [{"file":"assets/js/core/history.js","lines":[217,218]}] |
| remove_recent | [rust/readmd-kernel/src/main.rs:3398](../../../rust/readmd-kernel/src/main.rs) | ["/api/recent/remove"] | [{"file":"assets/js/core/history.js","lines":[25,26]}] |
| clear_recent | [rust/readmd-kernel/src/main.rs:3408](../../../rust/readmd-kernel/src/main.rs) | ["/api/recent/clear"] | [{"file":"assets/js/core/history.js","lines":[197,198]}] |
| check_recent_status | [rust/readmd-kernel/src/main.rs:3415](../../../rust/readmd-kernel/src/main.rs) | ["/api/recent/status"] | [{"file":"assets/js/core/history.js","lines":[54,56]}] |
| get_settings | [rust/readmd-kernel/src/main.rs:3434](../../../rust/readmd-kernel/src/main.rs) | ["/api/settings"] | [{"file":"assets/app.js","lines":[1293]},{"file":"assets/js/core/settings.js","lines":[27]}] |
| save_settings | [rust/readmd-kernel/src/main.rs:3446](../../../rust/readmd-kernel/src/main.rs) | ["/api/settings"] | [{"file":"assets/js/core/history.js","lines":[502]},{"file":"assets/js/core/settings.js","lines":[49]},{"file":"assets/js/core/state.js","lines":[163]}] |
| get_custom_styles | [rust/readmd-kernel/src/main.rs:3456](../../../rust/readmd-kernel/src/main.rs) | ["/api/style/get"] | [{"file":"assets/app.js","lines":[1328,1329]}] |
| save_custom_styles | [rust/readmd-kernel/src/main.rs:3462](../../../rust/readmd-kernel/src/main.rs) | ["/api/style/save"] | [{"file":"assets/app.js","lines":[1362,1363]}] |
| export_presentation | [rust/readmd-kernel/src/main.rs:3472](../../../rust/readmd-kernel/src/main.rs) | ["/api/export/presentation"] | [{"file":"assets/js/reader/render.js","lines":[2890,2891]}] |
| export_epub | [rust/readmd-kernel/src/main.rs:3482](../../../rust/readmd-kernel/src/main.rs) | ["/api/export/epub"] | [{"file":"assets/js/features/export.js","lines":[1315,1316]}] |
| export_doc | [rust/readmd-kernel/src/main.rs:3492](../../../rust/readmd-kernel/src/main.rs) | ["/api/export"] | [{"file":"assets/js/features/export.js","lines":[1325,1341,1342]}] |
| run_code_chunk | [rust/readmd-kernel/src/main.rs:3510](../../../rust/readmd-kernel/src/main.rs) | ["/api/code/run"] | [{"file":"assets/js/reader/render.js","lines":[1799,1800]}] |
| render_diagram | [rust/readmd-kernel/src/main.rs:3520](../../../rust/readmd-kernel/src/main.rs) | ["/api/diagram/render"] | [{"file":"assets/js/reader/render.js","lines":[2476,2477]}] |
| get_diagram_capabilities | [rust/readmd-kernel/src/main.rs:3540](../../../rust/readmd-kernel/src/main.rs) | ["/api/diagram/capabilities"] | [] |
| transcribe_file | [rust/readmd-kernel/src/main.rs:3546](../../../rust/readmd-kernel/src/main.rs) | ["/api/transcribe"] | [] |
| process_imports | [rust/readmd-kernel/src/main.rs:3565](../../../rust/readmd-kernel/src/main.rs) | ["/api/import/process"] | [{"file":"assets/js/reader/render.js","lines":[856,857]}] |
| extract_zip_batch | [rust/readmd-kernel/src/main.rs:3583](../../../rust/readmd-kernel/src/main.rs) | ["/api/batch/extract-zip"] | [] |
| get_links_graph | [rust/readmd-kernel/src/main.rs:3605](../../../rust/readmd-kernel/src/main.rs) | ["/api/links/graph?dir="] | [] |
| get_backlinks | [rust/readmd-kernel/src/main.rs:3623](../../../rust/readmd-kernel/src/main.rs) | ["/api/links/backlinks?path="] | [] |
| index_directory_links | [rust/readmd-kernel/src/main.rs:3629](../../../rust/readmd-kernel/src/main.rs) | ["/api/links/index"] | [] |
| get_bibtex | [rust/readmd-kernel/src/main.rs:3654](../../../rust/readmd-kernel/src/main.rs) | ["/api/bibtex?p="] | [{"file":"assets/js/reader/render.js","lines":[320,321]}] |
| check_update | [rust/readmd-kernel/src/main.rs:3702](../../../rust/readmd-kernel/src/main.rs) | ["/api/update/check"] | [{"file":"assets/js/features/updater.js","lines":[20,21]}] |
| start_download_update | [rust/readmd-kernel/src/main.rs:3732](../../../rust/readmd-kernel/src/main.rs) | ["/api/update/download"] | [{"file":"assets/js/features/updater.js","lines":[145,146]}] |
| get_download_status | [rust/readmd-kernel/src/main.rs:3742](../../../rust/readmd-kernel/src/main.rs) | ["/api/update/status"] | [{"file":"assets/js/features/updater.js","lines":[180,181]}] |
| cancel_download | [rust/readmd-kernel/src/main.rs:3748](../../../rust/readmd-kernel/src/main.rs) | ["/api/update/cancel"] | [{"file":"assets/js/features/updater.js","lines":[253,254]}] |
| apply_update | [rust/readmd-kernel/src/main.rs:3754](../../../rust/readmd-kernel/src/main.rs) | ["/api/update/apply"] | [{"file":"assets/js/features/updater.js","lines":[274]}] |
| get_system_language | [rust/readmd-kernel/src/main.rs:3764](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/language"] | [{"file":"assets/js/core/i18n.js","lines":[221,222]}] |
| get_app_info | [rust/readmd-kernel/src/main.rs:3771](../../../rust/readmd-kernel/src/main.rs) | [] | [] |
| check_upgrade | [rust/readmd-kernel/src/main.rs:3774](../../../rust/readmd-kernel/src/main.rs) | [] | [] |
| start_modules | [rust/readmd-kernel/src/main.rs:3777](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/core/modules.js","lines":[11]}] |
| get_modules_status | [rust/readmd-kernel/src/main.rs:3780](../../../rust/readmd-kernel/src/main.rs) | ["/api/modules"] | [] |
| get_pet_runtime_status | [rust/readmd-kernel/src/main.rs:3786](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/status"] | [] |
| configure_pet | [rust/readmd-kernel/src/main.rs:3792](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/configure"] | [] |
| interact_pet | [rust/readmd-kernel/src/main.rs:3802](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/interact"] | [] |
| get_pet_update_status | [rust/readmd-kernel/src/main.rs:3812](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/update_status"] | [] |
| check_pet_update | [rust/readmd-kernel/src/main.rs:3818](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/check_update"] | [] |
| apply_pet_update | [rust/readmd-kernel/src/main.rs:3828](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/apply_update"] | [] |
| install_companion_pet | [rust/readmd-kernel/src/main.rs:3834](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/install"] | [] |
| uninstall_companion_pet | [rust/readmd-kernel/src/main.rs:3840](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/uninstall"] | [] |
| list_local_pets | [rust/readmd-kernel/src/main.rs:3846](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets"] | [] |
| import_local_pet | [rust/readmd-kernel/src/main.rs:3852](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/import"] | [] |
| remove_local_pet | [rust/readmd-kernel/src/main.rs:3862](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/remove"] | [] |
| set_active_pet | [rust/readmd-kernel/src/main.rs:3872](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/active"] | [] |
| install_pet_plugin | [rust/readmd-kernel/src/main.rs:3882](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/install"] | [] |
| install_default_pet_plugin | [rust/readmd-kernel/src/main.rs:3888](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/runtime/install"] | [] |
| configure_pet | [rust/readmd-kernel/src/main.rs:3894](../../../rust/readmd-kernel/src/main.rs) | ["/api/pets/configure"] | [] |
| enqueue_pet_files | [rust/readmd-kernel/src/main.rs:3904](../../../rust/readmd-kernel/src/main.rs) | ["/api/control/pet-batch"] | [] |
| get_pet_batch | [rust/readmd-kernel/src/main.rs:3914](../../../rust/readmd-kernel/src/main.rs) | ["/api/control/pet-batch"] | [] |
| render_web_page | [rust/readmd-kernel/src/main.rs:3920](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/features/web.js","lines":[93,104]}] |
| cancel_web_render | [rust/readmd-kernel/src/main.rs:3923](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/features/web.js","lines":[146]}] |
| authorize_private_web | [rust/readmd-kernel/src/main.rs:3924](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/features/web.js","lines":[171,172]}] |
| revoke_private_web | [rust/readmd-kernel/src/main.rs:3927](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/features/web.js","lines":[147,235]}] |
| rename_file | [rust/readmd-kernel/src/main.rs:3941](../../../rust/readmd-kernel/src/main.rs) | ["/api/rename"] | [{"file":"assets/js/core/tabs.js","lines":[377,380]},{"file":"assets/js/reader/render.js","lines":[86,88]}] |
| get_export_presets | [rust/readmd-kernel/src/main.rs:3951](../../../rust/readmd-kernel/src/main.rs) | ["/api/export/presets"] | [{"file":"assets/js/features/export.js","lines":[185,186]}] |
| save_export_presets | [rust/readmd-kernel/src/main.rs:3957](../../../rust/readmd-kernel/src/main.rs) | ["/api/export/presets"] | [{"file":"assets/js/features/export.js","lines":[1392,1449,1450]}] |
| open_path | [rust/readmd-kernel/src/main.rs:3970](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/open-path"] | [{"file":"assets/js/features/export.js","lines":[1388]},{"file":"assets/js/reader/render.js","lines":[3026]}] |
| open_external | [rust/readmd-kernel/src/main.rs:3999](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/open-path"] | [{"file":"assets/app.js","lines":[271]},{"file":"assets/js/reader/render.js","lines":[2986]}] |
| reveal_path | [rust/readmd-kernel/src/main.rs:4010](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/reveal-path"] | [{"file":"assets/js/features/export.js","lines":[1389]}] |
| open_dir | [rust/readmd-kernel/src/main.rs:4020](../../../rust/readmd-kernel/src/main.rs) | ["/api/system/open-path"] | [{"file":"assets/app.js","lines":[136]}] |
| report_ready | [rust/readmd-kernel/src/main.rs:4027](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/app.js","lines":[1279]}] |
| show_window | [rust/readmd-kernel/src/main.rs:4039](../../../rust/readmd-kernel/src/main.rs) | [] | [{"file":"assets/js/reader/render.js","lines":[250,251]}] |
| toggle_native_fullscreen | [rust/readmd-kernel/src/main.rs:4052](../../../rust/readmd-kernel/src/main.rs) | [] | [] |
| request_quit | [rust/readmd-kernel/src/main.rs:4062](../../../rust/readmd-kernel/src/main.rs) | [] | [] |

render_web_page、cancel_web_render、authorize_private_web、revoke_private_web均由Rust桌面桥nativeWeb IPC实现，不经过HTTP路由；隔离网页窗口只接收提取消息，不注入阅读器API对象。内核目前没有系统托盘，不从API推导“托盘菜单”入口。控制队列、ping/status等基础设施由现有UI间接使用，未列成独立按钮。
