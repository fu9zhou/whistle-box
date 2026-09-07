> 此文件是修复前的历史基线，行号和失败结果对应当时源码。当前修复与验证结果见 [修复验收报告](FIX-VALIDATION-2026-09-07.md)。

# WhistleBox 项目检查报告

检查日期：2026-09-07。范围：当前工作区全部业务源码、前后端状态与调用链、Whistle sidecar、系统代理/PAC、认证代理、证书、配置与规则、首次引导、托盘、自启、安装卸载、构建与官网说明。开始检查时 Git 工作区无修改；本次仅新增审计文件，没有修改业务代码。

## 结论与证据边界

目前不能认为项目能安全地与用户已有 Whistle 共存。默认内置端口为 **18899**，外部端口为 **8899**，内置数据目录使用 `.WhistleBoxData` 和 `-S whistlebox_embedded`，这些隔离措施是存在的；问题主要出在停止/清理范围、证书归属、系统代理所有权，以及多个不同步的运行状态。

**“电脑安装了全局 Whistle”本身不必然导致冲突。** 高风险条件是：已有实例正在运行、端口重叠、选择外部连接、两套实例使用不同 CA、安装卸载触及共享数据目录。项目并没有可靠处理这些情况。

下文记录 **29 条问题与风险**，其中前端 **7 个失败场景已用隔离脚本复现，2 个对照检查通过**。标为“代码确认”的项目有明确的执行路径，但没有执行真实杀进程、系统代理变更、证书删除和安装卸载操作。

**尚未复现真实 WebView2 白屏。** 不能把本报告中的某一个缺陷宣称为你机器上全部白屏的唯一原因。白屏相关的确定缺陷包括持续重建 iframe、内置模式缺少状态刷新，以及“代理端口活着”被误认为“界面已就绪”。

环境限制：

- Node 可用，版本 `v22.22.2`；初始缺少 `node_modules`、内嵌 Whistle 依赖、sidecar 与 Rust 构建产物。
- PATH 中无 `cargo`、`rustc`、`wmic`；常见 `.cargo/bin/cargo.exe` 也不存在。没有进行 Rust 编译或原生应用启动。
- 多次 `npm ci --ignore-scripts --no-audit --no-fund` 均以 `Exit handler never called!` 失败；尝试独立缓存和扩展执行权限也未完成。单独访问 npm registry 收到套接字权限错误。因此不能将本环境的依赖安装失败归因于项目本身。
- `npm run build` 失败于 `tsc is not recognized`，不是已证实的 TypeScript 编译错误。
- PATH 未找到 `w2/whistle`；只读监听查询未返回四个默认端口的结果。这不能排除用户在其他路径安装或运行了 Whistle。
- 未读取用户凭据、修改系统代理、安装/删除 CA、停止现有 Node 进程，未执行重置或安装卸载脚本。

优先级：**P1** 应在继续发布或依赖其管理系统网络前修复；**P2** 是功能正确性、稳定性或防护缺陷，应列入下一批修复。

## 一、与已有 Whistle 的冲突

### 01 · P1 · 外部模式退出也会停止用户自己的实例【代码确认】

- 位置：[cmd_stop_whistle](D:/whistle-box/src-tauri/src/whistle/mod.rs:509)、[托盘退出](D:/whistle-box/src-tauri/src/tray.rs:218)、[窗口退出](D:/whistle-box/src-tauri/src/lib.rs:225)。
- 触发：连接外部 Whistle，随后从托盘退出，或关闭窗口且未启用最小化到托盘。
- 原因：停止命令没有 `external` 模式保护；取 `active_endpoint()` 后发送 stop 请求，本地地址还进入端口杀进程逻辑。即便从未启动内置进程，也会使用外部配置端口。
- 影响：WhistleBox 只是连接客户端，却可能停止用户独立运行的 Whistle。远端 stop 接口是否支持还需核对锁定版本，但本地端口清理路径已经明确。
- 修复：把“断开外部连接”与“停止本应用创建的内置实例”完全分开，退出只回收本应用拥有的资源。

### 02 · P1 · 端口冲突处理可误杀其他 Node 服务【代码确认】

- 位置：[进程识别](D:/whistle-box/src-tauri/src/utils.rs:38)、[启动清理](D:/whistle-box/src-tauri/src/whistle/mod.rs:277)、[认证代理抢占端口](D:/whistle-box/src-tauri/src/auth/mod.rs:393)。
- 判断归属只需进程描述包含 `node`、`whistle` 或 `whistlebox`。这不是实例归属校验，任意 Node 服务都可能被选中。
- Whistle 启动前直接清理目标端口，并重试杀进程；Auth Proxy 绑定失败也会清理端口。配置错端口即可影响已有 Whistle、Vite 或其他 Node 服务。
- 当前环境无 `wmic`，这条 Windows 查询路径还会失效，表现为清理无效、启动失败；不能据此认为其他机器不会被误杀。
- 修复：外部占用应返回冲突详情或选择空闲端口；只使用受控子进程句柄、已验证的 PID/启动时间/路径停止自有实例。

### 03 · P1 · 启动和退出无条件清空系统代理【代码确认】

- 位置：[启动清理](D:/whistle-box/src-tauri/src/lib.rs:121)、[clear_system_proxy](D:/whistle-box/src-tauri/src/proxy/mod.rs:86)。
- 即使 `auto_start_proxy=false`、首次引导未完成、使用外部模式，也会清空原有 ProxyServer 和 PAC URL；退出同样清理。
- 影响：已有 Whistle、其他代理客户端或企业 PAC 的配置被破坏，没有保存并恢复原配置。
- 修复：取得代理所有权前保存快照；只有当前系统值仍等于本应用设置的值时才恢复，避免覆盖用户或其他客户端后续修改。

### 04 · P1 · CA 检查与删除没有实例归属【代码确认】

- 位置：[CA 检查](D:/whistle-box/src-tauri/src/whistle/mod.rs:707)、[CA 删除](D:/whistle-box/src-tauri/src/whistle/mod.rs:736)。
- 检查仅搜索证书列表中的 `whistle/rootca` 字符串，没有对比当前实例根证书的指纹。
- 删除遍历所有包含 `whistle` 的证书块，对提取到的指纹逐一删除。
- 影响：全局 Whistle 的 CA 可以让新内置实例被误判为已安装，随后 HTTPS 抓包仍报证书错误；点击移除可能同时破坏其他 Whistle 的 HTTPS 信任。
- 修复：下载/解析当前实例 CA，以指纹核验；持久记录本应用安装过的证书，只删除明确归属本应用的指纹。

### 05 · P1 · 安装卸载可以删除全局 Whistle 数据【代码确认】

- 位置：[安装阶段](D:/whistle-box/src-tauri/nsis-hooks.nsi:104)、[卸载阶段](D:/whistle-box/src-tauri/nsis-hooks.nsi:177)。
- 安装器把 `$PROFILE\.whistle` 作为清理对象；用户在“是否保留”询问中选否后会递归删除。
- **不是静默删除**，但这一目录不属于 WhistleBox 专有数据。提示没有明确说明可能删除另行安装的 Whistle 的规则、配置和证书材料。
- 修复：安装器只管理本应用专属目录；全局目录迁移必须是独立、明确、可备份的功能。

### 06 · P1 · 可将系统代理指向未就绪或已停止的 Whistle【代码确认】

- 位置：[系统代理切换](D:/whistle-box/src-tauri/src/proxy/mod.rs:126)、[代理页面](D:/whistle-box/src/components/proxy/ProxyControl.tsx:75)、[自动恢复](D:/whistle-box/src-tauri/src/lib.rs:187)、[托盘切换](D:/whistle-box/src-tauri/src/tray.rs:282)。
- 后端切换不检查服务可用性；外部启动命令在连接失败时返回 `Ok(running=false)`，前端仍可继续切换代理。
- 自动恢复代理没有以 Whistle 启动成功为条件；托盘忽略 PAC 启动错误；停止 Whistle 也不撤销当前代理。
- 影响：端口冲突、凭据错误或服务停止后，系统仍指向不可用端点，造成断网。
- 修复：服务启动、健康确认、PAC 就绪和设置系统代理作为一个可回滚流程；停止或恢复失败时按所有权恢复网络配置。

## 二、白屏、启动与状态

### 07 · P1 · 健康 iframe 每 6 秒被重建【隔离复现 F01】

- 位置：[重试 effect](D:/whistle-box/src/components/whistle/WhistleView.tsx:113)、[iframe key](D:/whistle-box/src/components/whistle/WhistleView.tsx:339)。
- 定时器在探测成功后增加 `iframeKey`，而 `iframeKey` 又是 effect 依赖，形成持续重载。没有“已加载成功即停止”的条件。
- 结果：虚拟时间 18 秒内 key 改变 **3 次**。网络较慢时可能反复打断资源加载；正常情况下也会丢失界面交互状态。
- 修复：成功后不自动重建；失败恢复应有明确错误信号、有限重试和成功终止条件。

### 08 · P1 · 内置页面不会持续更新启动/恢复状态【隔离复现 F02】

- 位置：[首次状态查询](D:/whistle-box/src/components/whistle/WhistleView.tsx:140)、[只为外部模式轮询](D:/whistle-box/src/components/whistle/WhistleView.tsx:171)、[Dashboard 轮询](D:/whistle-box/src/components/dashboard/Dashboard.tsx:39)。
- 打开内置页面时只查询一次；Dashboard 离开后被卸载，轮询也随之停止。后端健康监测没有向前端发送状态事件。
- 结果：模拟后台启动完成后再过 60 秒，内置页面仍只查询 **1 次**；外部模式对照组同样 60 秒内正常执行 **4 次**轮询。
- 影响：启动完成仍显示未启动，或进程崩溃后 iframe 仍显示旧的可用状态。
- 修复：把状态同步放到应用级 store/事件订阅中，页面只消费状态。

### 09 · P2 · readiness 检查与真实界面可用性脱节【代码确认】

- 位置：[Whistle 检查](D:/whistle-box/src-tauri/src/whistle/mod.rs:49)、[Auth Proxy health](D:/whistle-box/src-tauri/src/auth/mod.rs:128)、[前端探测](D:/whistle-box/src-tauri/src/auth/mod.rs:490)。
- Whistle 检查只认 HTTP 2xx，不校验响应身份；Auth Proxy `/__health` 无条件返回 200，不访问上游、不验证凭据或 HTML/资源。
- 影响：上游失效、认证失败、目标接错时仍可显示“界面就绪”；探测无法解释白屏，也无法触发正确恢复。
- 修复：区分进程存活、端口监听、Whistle 身份、认证成功、UI 资源加载完成等状态，并保留具体错误。

### 10 · P1 · 后台自动启动认证代理用了内置端点【代码确认】

- 位置：[后台启动 Auth Proxy](D:/whistle-box/src-tauri/src/lib.rs:174)。
- 这里直接使用 `conn.host/port/username/password`，没有使用 `active_endpoint()/active_credentials()`。
- 触发：外部模式启动，或启动期间修改模式/端点。后台流程可能把前端刚设置好的认证代理重新指向内置实例；它保存的是较早的配置快照。
- 修复：使用统一的运行目标解析逻辑；异步启动应携带配置版本，旧任务不得覆盖新配置。

### 11 · P1 · 跟踪启动器 PID，且健康检查失败仍报告启动成功【代码确认】

- 位置：[后台启动命令](D:/whistle-box/src-tauri/src/whistle/mod.rs:316)、[保存 PID](D:/whistle-box/src-tauri/src/whistle/mod.rs:406)、[失败后仍 Ok](D:/whistle-box/src-tauri/src/whistle/mod.rs:454)。
- `start` 是后台模式，初始 Child PID 不等同于守护进程 PID。健康检查成功的常见路径直接返回初始 PID；找实际监听 PID 的逻辑只在检查超时后执行。
- 所有检查失败后最终仍 `Ok(pid)`，调用方设置 `running=true`。端口扫描找到任意监听者也被当作启动成功。
- 影响：状态失真、关闭遗漏、重启依赖粗暴端口清理，过期 PID 后续还可能被复用。
- 修复：采用可持有句柄的前台子进程或可靠的守护进程控制协议；超时必须报错并清理本次创建的资源。
- 语义依据：[Whistle 官方命令文档](https://wproxy.org/whistle/proxy.html)区分 `run` 前台与 `start` 后台；具体 2.10.2 守护实现仍需锁定包验证。

### 12 · P1 · 自动启动、手动启停、健康重启之间没有生命周期互斥【代码确认】

- 位置：[后台启动](D:/whistle-box/src-tauri/src/lib.rs:136)、[健康恢复](D:/whistle-box/src-tauri/src/whistle/mod.rs:62)、[启动实现](D:/whistle-box/src-tauri/src/whistle/mod.rs:277)。
- 只有独立 PID/端口原子变量及布尔锁，没有包住整个 start/stop/restart 的操作锁、取消令牌或 generation。
- 首次启动也不检查 `setup_completed`，向导期间就可能启动内置实例。手动启动与延时自动启动相遇，会互相清理进程；监测已经进入恢复流程后，用户停止并不能取消它。
- 修复：单一生命周期状态机，串行处理命令，停止使旧启动任务失效；初次引导完成前不自动创建实例。

### 13 · P1 · 没有校验内部端口互相冲突【代码确认】

- 位置：[配置校验](D:/whistle-box/src-tauri/src/config/mod.rs:307)。
- Whistle/Auth/PAC 只校验非零，没有检查两两冲突及 SOCKS 端口冲突。
- 若 Auth 与 Whistle 同端口，抢占路径可终止刚启动的 Whistle；若 Whistle 占用应用内 Auth/PAC 监听端口，粗糙的进程归属判断甚至可以选中 WhistleBox 自身。
- 修复：区分内置模式和外部模式，校验实际本地监听地址/端口组合；冲突只报错，绝不尝试夺取。

### 14 · P2 · 端口配置和实际 Auth/PAC 监听端口可不一致【代码确认】

- 位置：[Auth 配置热更新](D:/whistle-box/src-tauri/src/auth/mod.rs:335)、[PAC 已运行分支](D:/whistle-box/src-tauri/src/proxy/pac.rs:173)。
- Auth 在已监听时仅更新配置，直接返回成功，但调用方会发布新端口 URL；旧监听没有迁移。PAC 在已运行时也直接返回配置中的新端口。
- 设置界面虽提示重启应用，但重启前的其他配置更新、页面进入或导入配置仍可触发这些分支。
- 修复：明确区分 desired 配置与 active 配置；未完成实际重新绑定前，所有 URL 和鉴权校验必须使用真实监听端口。

## 三、配置、规则与用户操作

### 15 · P1 · 保存、导入导出等失败被吞，界面可以显示成功【隔离复现 F03/F04】

- 位置：[saveConfig](D:/whistle-box/src/stores/appStore.ts:182)、[quietly](D:/whistle-box/src/stores/appStore.ts:143)、[导入导出反馈](D:/whistle-box/src/components/config/ConfigManager.tsx:171)。
- Store 捕获异常后只写全局 error，不再抛出。调用方 `await` 正常结束，继续显示“保存成功/导出成功”，还可能更新 savedForm、重启标志或继续启停操作。
- 注入保存/导出拒绝，两次预期 rejection 均未出现。导入、规则导出及 Auth Proxy 启动共享同类包装。
- 修复：操作返回明确 Result 或拒绝 Promise；只有成功才提交 UI 状态。全局错误提示不能替代调用链错误传播。

### 16 · P1 · 自动保存等待期间切换页面会直接丢修改【隔离复现 F05/F06】

- 位置：[规则卸载清理](D:/whistle-box/src/components/rules/RuleEditor.tsx:39)、[规则延迟保存](D:/whistle-box/src/components/rules/RuleEditor.tsx:76)、[设置卸载清理](D:/whistle-box/src/components/settings/useSettingsForm.ts:124)。
- 规则与设置均延迟 800ms 保存；卸载时只取消计时器，没有提交待保存内容。
- 结果：新增规则或改端口后立刻切页，后端保存调用均为 **0**。对照组保持页面打开超过 800ms，保存正常执行 **1 次**。
- 修复：把保存队列移到页面外，或离开前 flush；显示明确的待保存/失败状态。

### 17 · P2 · 只修改外部用户名/密码不会更新认证代理【隔离复现 F07】

- 位置：[changedProxyTarget](D:/whistle-box/src/components/settings/useSettingsForm.ts:236)。
- 变更检测包括外部 host/port，却遗漏 `externalUsername/externalPassword`。测试连接用新凭据，认证代理仍持有旧凭据。
- 结果：改外部密码后配置保存 **1 次**、Auth Proxy 更新 **0 次**。
- 修复：以完整 active target（模式、host、port、用户名、密码）作为一致的配置变更单元。

### 18 · P1 · 切换模式/导入配置后运行资源没有统一应用【代码确认】

- 位置：[模式切换](D:/whistle-box/src/components/settings/useSettingsForm.ts:359)、[导入 store](D:/whistle-box/src/stores/appStore.ts:280)、[后端导入](D:/whistle-box/src-tauri/src/config/mod.rs:385)。
- 模式切换更新配置和 Auth Proxy，但没有同步已经启用的全局代理/PAC 目标；切到外部还会先停内置，系统可能继续指向原内置端口。
- 导入只替换 config 并刷新 PAC，不统一处理 Whistle、Auth、实际系统代理、重启提示；后端导入也未同步 `minimize_to_tray` 原子值。
- 影响：UI 新配置、系统网络和进程运行目标互不一致；导入后关闭行为也可能仍遵循旧值。
- 修复：统一 `apply_config`，按差异事务化应用运行资源，失败回滚；配置文件与实际运行状态分别呈现。

### 19 · P2 · 删除当前 Profile 后 PAC 仍保留旧规则【代码确认】

- 位置：[删除 Profile](D:/whistle-box/src/components/config/ConfigManager.tsx:71)。
- 删除当前配置时改选另一个 Profile 并调用 saveConfig，没有像 switchProfile 那样调用 refreshPac。
- 影响：界面已选新配置，系统规则代理继续执行被删除配置的规则。
- 修复：所有会改变有效 PAC 的配置操作统一触发 PAC 更新。

### 20 · P2 · 配置保存不是跨调用串行事务【代码确认】

- 位置：[保存文件](D:/whistle-box/src-tauri/src/config/mod.rs:210)、[后端保存命令](D:/whistle-box/src-tauri/src/config/mod.rs:366)、[代理模式持久化](D:/whistle-box/src-tauri/src/proxy/mod.rs:262)。
- `config.save()` 在取得配置锁之前执行，使用固定 `config.json.tmp`。不同命令/后台任务可并发写同一临时文件，产生覆盖或 rename 失败。
- 前端每个页面的局部队列不能保护后台托盘/代理模式写入；全量 config 快照保存还可覆盖其他命令刚更新的字段。
- 修复：后端单一写入锁与版本检查，采用字段级变更或统一事务；写盘成功后原子提交内存状态。

### 21 · P1 · 校验失败的旧配置会被默认配置覆盖且不备份【代码确认】

- 位置：[load 校验](D:/whistle-box/src-tauri/src/config/mod.rs:176)、[启动默认覆盖](D:/whistle-box/src-tauri/src/lib.rs:46)。
- JSON 反序列化失败分支有备份，但“JSON 可解析、配置校验失败”直接返回 Err。启动层随后保存默认配置，覆盖原文件。
- 触发：旧版本配置与新校验规则不兼容、Profile 引用失效、用户手动修改了某字段等。
- 修复：先备份再迁移/修复；恢复失败保持原文件，避免因一个字段失效丢掉所有 Profile 和连接信息。

## 四、认证代理及其他稳定性

### 22 · P2 · Referer 中出现 `_token=` 就能通过鉴权【代码确认】

- 位置：[is_authorized_request](D:/whistle-box/src-tauri/src/auth/mod.rs:65)。
- URL query 的 token 会检查实际值，但 Referer 分支只检查字符串包含 `_token=`；`tauri://` 前缀也直接允许。
- 可伪造请求头的本地客户端无须知道真实 token，就能通过这些分支并使用代理注入的上游身份。
- 这是回环服务的鉴权缺陷，**没有据此证明互联网远程攻击可直接利用**。
- 修复：实际校验 token 或建立受控 session；Origin/Referer 只能作为辅助约束，不能当作秘密凭证。

### 23 · P2 · 鉴权没有持久会话，二级资源请求可被误拒绝【代码确认，浏览器链路待验证】

- 位置：[认证策略](D:/whistle-box/src-tauri/src/auth/mod.rs:65)。
- 顶层 HTML token URL 可以通过，但请求若仅携带不含 token 的同源 Referer（如样式表引用字体/图片、插件 iframe 内页面），`from_self` 并不能独立放行，最终返回 403。
- 不存在 session cookie 或其他能延续合法会话的机制。
- 是否是你当前 Whistle 2.10.2 的关键阻塞资源，需用实际资源 URL、Referer 和状态码确定；本次未获取到该版本的完整资源包，不将其当作已复现白屏根因。
- 修复：使用有有效期的本地 session，同时修复第 22 项，不能以放开所有请求代替。

### 24 · P2 · 认证转发全量缓冲且没有明确超时/大小上限【代码确认】

- 位置：[client](D:/whistle-box/src-tauri/src/auth/mod.rs:319)、[请求 collect](D:/whistle-box/src-tauri/src/auth/mod.rs:188)、[响应 bytes](D:/whistle-box/src-tauri/src/auth/mod.rs:234)。
- 上传和下载全部收集到内存，部分路径再复制；请求体和响应体没有大小限制，client 没有配置超时，服务连接也没有空闲时限。
- 128 并发信号量只限制连接数，不限制总字节数或挂起时长。大抓包导出、挂住的上游或慢连接可造成高内存/连接槽耗尽。
- 修复：流式转发、合理的请求/读取/空闲超时和大小上限，并传递真实读取错误。WebSocket/Upgrade 兼容性另列待测。

### 25 · P2 · 向导返回修改设置后可能不重新启动实例【代码确认】

- 位置：[configSaved](D:/whistle-box/src/components/setup/SetupWizard.tsx:65)、[saveAndStart](D:/whistle-box/src/components/setup/SetupWizard.tsx:69)。
- 一次保存后 `configSaved=true`，返回修改模式、地址、凭据不会清除这个标志。
- 再进入步骤可以直接跳过保存；最终完成虽会写配置，但 `!configSaved` 的启动/认证代理刷新分支被跳过。首次启动失败也被捕获后继续置为已保存。
- 修复：跟踪配置版本与启动结果；配置变化必须失效旧启动结果，失败应停留在可重试步骤。

### 26 · P2 · sidecar 只检查依赖目录存在，可能打包旧版本【代码确认】

- 位置：[prepare-sidecar](D:/whistle-box/scripts/prepare-sidecar.mjs:177)。
- `node_modules/whistle` 存在就跳过 `npm ci`，不检查当前版本、锁文件变化或安装完整性。
- 影响：更新锁文件后再次开发/打包仍可能使用旧 Whistle，开发机之间表现不一致。
- 修复：构建执行锁定安装，或验证锁文件指纹及安装完整性后复用缓存；清理自引用 junction 的补丁不能替代依赖一致性验证。

### 27 · P2 · 启动错误输出被丢弃，缺少可持久诊断信息【代码确认】

- 位置：[子进程标准输出/错误](D:/whistle-box/src-tauri/src/whistle/mod.rs:388)、[日志初始化](D:/whistle-box/src-tauri/src/lib.rs:44)。
- Whistle stdout/stderr 均为 null；应用仅 env_logger 初始化，没有文件日志/诊断导出配置。
- 影响：参数错误、端口占用、数据目录权限等最有价值的错误信息被丢掉，用户只能看到笼统“无响应/连接失败”。
- 修复：保存有上限、轮转、脱敏的启动与请求失败日志，记录启动退出码、真实 PID、实际监听端口及 UI 失败资源。

### 28 · P2 · 校验允许 IPv6，但 URL 构造不支持【代码确认】

- 位置：[允许 loopback](D:/whistle-box/src-tauri/src/config/mod.rs:271)、[健康 URL](D:/whistle-box/src-tauri/src/whistle/mod.rs:50)、[前端 URL](D:/whistle-box/src/components/whistle/WhistleView.tsx:31)。
- `::1` 会通过配置校验，却被拼成 `http://::1:端口/...`，没有 IPv6 所需的方括号；前端安全检查对 URL 标准化后 hostname 的处理也不一致。
- 影响：合法配置无法连接，且重试/重启不能解决。
- 修复：统一使用结构化 URL/SocketAddr 构造；补充 IPv4、localhost、IPv6 的一致性测试。

### 29 · P1 · 强制卸载/重置缺少系统网络及自启清理【代码确认】

- 位置：[NSIS hooks](D:/whistle-box/src-tauri/nsis-hooks.nsi:109)、[重置脚本](D:/whistle-box/scripts/reset-config.ps1:7)、[自启注册](D:/whistle-box/src-tauri/src/autostart.rs:15)。
- 安装/卸载与重置强制终止应用/sidecar，绕过应用正常退出清理；这些脚本没有按本应用归属恢复系统代理。自定义 Run 键 `WhistleBox` 的清理也未见于自定义卸载 hook。
- 影响：代理启用时卸载/重置可以留下指向已停止服务的代理或 PAC；自启残留需结合生成的 NSIS 脚本进一步核实。
- 修复：强退前走受控 shutdown，失败后仅清理由本应用设置的代理；显式移除本应用的自启项。不得恢复为第 03 项的无条件清空。

## 五、白屏后续验证顺序

按现有证据，优先验证以下三个方向，不把推断当成现场结论：

1. **iframe 生命周期**：去除成功状态下的 6 秒重建后，若页面可以稳定加载，说明它参与了白屏/闪烁。需保留慢速加载和正常加载两组对照。
2. **启动与配置状态不同步**：记录实际 Whistle、Auth Proxy 端点及前端状态版本；如果卡住时后台已就绪而前端未更新，补应用级同步后应恢复。
3. **认证代理资源链路**：记录首页与 JS/CSS/字体/CGI 的状态、Content-Type、实际 Referer（脱敏）；若首页 200 但关键资源 403/502，则按鉴权和目标端点分别定位。

后续还应测试：用户已有 `.whistlerc`/Whistle 环境变量对 2.10.2 的影响、独立 UI 端口、Whistle import/export/rootca/stop 接口契约、上游代理设置是否真正作用于转发请求、Whistle 插件与 WebSocket/Upgrade、WebView2 Referer/CSP/缓存行为、安装器实际打包资源与目标架构。上述项目本次**没有足够运行证据**，不列为已证实根因。

当前官方文档记载 Whistle 会读取用户级启动配置并支持 UI 独立端口，见[命令行配置说明](https://wproxy.org/docs/cli.html)；文档可能对应比锁定的 2.10.2 更新的版本，因此不能直接认定本项目受到这些配置污染。

## 六、复现命令与结果

在项目根目录，Node 22.13+ 可执行：

```powershell
node --disable-warning=ExperimentalWarning audit/reproduce.mjs
```

脚本直接提取当前 TypeScript 业务函数，使用 Node 内置类型剥离和 VM，mock IPC、Hook 调度和虚拟时间；不依赖 npm 安装，不接触真实网络或系统设置。它验证的是业务分支及 effect 行为，**不等同于 React DOM、Rust 或 WebView2 集成测试**。

| 场景 | 结果 | 观察 |
|---|---|---|
| F01 健康 iframe 应保持稳定 | FAIL | 18 秒重建 3 次 |
| F02 后台启动后内置状态应更新 | FAIL | 60 秒总共只查询 1 次 |
| F03 保存失败应拒绝调用 | FAIL | Promise 正常返回 |
| F04 导出失败应拒绝调用 | FAIL | Promise 正常返回 |
| F05 新增规则后立即离开应保留修改 | FAIL | 保存 0 次 |
| F06 改设置后立即离开应保留修改 | FAIL | 保存 0 次 |
| F07 修改外部密码应刷新 Auth Proxy | FAIL | 保存 1 次、代理更新 0 次 |
| 对照：规则页面等待保存完成 | PASS | 保存 1 次 |
| 对照：外部页面保持打开 | PASS | 60 秒额外轮询 4 次 |

脚本故意对正确行为做断言，当前版本退出码为 **1**；这是发现缺陷的结果，不是测试脚本安装失败。

## 七、修复与验收建议

建议顺序：

1. **先保护已有环境**：修复 01–06、13、29。消除误杀、共享目录删除、跨实例 CA 删除和系统代理覆盖。
2. **再统一启动与界面状态**：修复 07–14、18，使用一个生命周期管理器和实际运行配置，建立可诊断的 readiness。
3. **修复保存与配置应用**：15–21、25，统一后端事务与前端错误传播，保证页面退出不丢修改。
4. **完善转发与发布验证**：22–24、26–28；加入 Windows 集成测试、安装升级卸载测试和真实 WebView2 检查。

最低验收矩阵应覆盖：纯净机器；已安装但未启动全局 Whistle；全局 Whistle 已监听 8899；内置端口/Auth/PAC/SOCKS 分别被占；外部 Whistle 有/无认证；两实例 CA 不同；启动中切模式/退出；断线恢复；全局和 PAC 模式下停止/退出/卸载；配置保存失败；快速切页；配置导入和活动 Profile 删除。

每次共存测试都应断言：**既有进程存活、既有规则目录未变、非本应用 CA 未变、退出后网络配置正确恢复、内置 UI 无周期性重载**。

仓库目前没有测试命令、现有测试用例或 CI 工作流。构建、原生运行、协议兼容性与安装卸载验收仍需在依赖与 Rust 工具链可用的 Windows 环境完成。
