// WhistleBox owns this foreground process. Configuration travels over stdin,
// so credentials never appear in a command line or a daemon configuration file.
const readline = require('node:readline');
const input = readline.createInterface({ input: process.stdin });
input.once('line', (line) => {
  let options;
  try {
    options = JSON.parse(line);
    process.env.WHISTLE_PATH = options.baseDir;
    for (const key of Object.keys(process.env)) {
      if (/^(WHISTLE_|W2_|PFORK_|STARTING_)/i.test(key) && key !== 'WHISTLE_PATH') delete process.env[key];
    }
    const secrets = [options.username, options.password, options.upstream_proxy].filter(Boolean);
    const report = (error) => {
      let message = error?.message || String(error);
      for (const secret of secrets) message = message.split(secret).join('[redacted]');
      console.error(message.slice(0, 2048));
      process.exitCode = 1;
      process.exit(1);
    };
    process.on('uncaughtException', report);
    process.on('unhandledRejection', report);
    const config = {
      host: options.host, port: options.port, username: options.username, password: options.password,
      baseDir: options.baseDir, storage: 'whistlebox_embedded', rcPath: 'none', noGlobalPlugins: true,
      socksPort: options.socks_port || undefined, timeout: (options.timeout || 60) * 1000,
      mode: options.intercept_https ? 'capture' : '',
    };
    if (options.upstream_proxy) {
      const upstream = new URL(options.upstream_proxy.includes('://') ? options.upstream_proxy : 'http://' + options.upstream_proxy);
      const protocol = { 'http:': 'proxy', 'https:': 'https-proxy', 'socks5:': 'socks' }[upstream.protocol];
      if (!protocol) throw new Error('Unsupported upstream proxy scheme');
      config.shadowRules = '* ' + protocol + '://' + upstream.href.split('://')[1];
    }
    require('./node_modules/whistle')(config, () => console.log('WhistleBox embedded instance ready'));
  } catch (error) {
    console.error('WhistleBox failed to initialize its embedded instance');
    process.exit(1);
  }
});
// If the application crashes or exits, the pipe closes and the child exits too.
input.on('close', () => process.exit(0));
process.on('SIGTERM', () => process.exit(0));
