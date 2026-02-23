#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

const binaryName = process.platform === 'win32' ? 'hermez.exe' : 'hermez';
const binaryPath = path.join(__dirname, 'bin', binaryName);

const child = spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit' });

child.on('exit', (code) => {
  process.exit(code ?? 0);
});