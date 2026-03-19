const fs = require('fs');
const path = require('path');
const os = require('os');

const platform = os.platform();
const arch = os.arch();

const PACKAGE_MAP = {
  'linux-x64':    '@hermez-tunnels/cli-linux-x64',
  'linux-arm64':  '@hermez-tunnels/cli-linux-arm64',
  'darwin-x64':   '@hermez-tunnels/cli-darwin-x64',
  'darwin-arm64': '@hermez-tunnels/cli-darwin-arm64',
  'win32-x64':    '@hermez-tunnels/cli-win32-x64',
};

const key = `${platform}-${arch}`;
const packageName = PACKAGE_MAP[key];

if (!packageName) {
  console.error(`hermez: unsupported platform: ${key}`);
  console.error('Supported platforms: linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64');
  process.exit(1);
}

const binaryName = platform === 'win32' ? 'hermez.exe' : 'hermez';

let packageDir;
try {
  packageDir = path.dirname(require.resolve(`${packageName}/package.json`));
} catch {
  console.error(`hermez: could not find package ${packageName}`);
  console.error('Make sure optional dependencies are not disabled (--no-optional).');
  process.exit(1);
}

const src = path.join(packageDir, 'bin', binaryName);
const dest = path.join(__dirname, 'bin', binaryName);

if (!fs.existsSync(src)) {
  console.error(`hermez: could not find binary at ${src}`);
  console.error('Make sure optional dependencies are not disabled (--no-optional).');
  process.exit(1);
}

fs.mkdirSync(path.join(__dirname, 'bin'), { recursive: true });
fs.copyFileSync(src, dest);
fs.chmodSync(dest, 0o755);

console.log(`hermez: installed binary for ${key}`);