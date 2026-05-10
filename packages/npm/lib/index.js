/**
 * rjest - A fast Jest-compatible test runner written in Rust
 */

const path = require('path');
const { spawn } = require('child_process');

function getBinaryPath() {
  const os = require('os');
  const binPath = path.join(__dirname, '..', 'bin');

  if (os.platform() === 'win32') {
    return path.join(binPath, 'jest.exe');
  }
  return path.join(binPath, 'jest');
}

/**
 * Run rjest with the given arguments
 * @param {string[]} args - Arguments to pass to jest
 * @param {object} options - Options for spawn
 * @returns {Promise<number>} Exit code
 */
function run(args = [], options = {}) {
  return new Promise((resolve, reject) => {
    const binaryPath = getBinaryPath();

    if (!require('fs').existsSync(binaryPath)) {
      console.error('rjest binary not found. Try running "npm install" first.');
      process.exit(1);
    }

    const child = spawn(binaryPath, args, {
      stdio: 'inherit',
      ...options,
    });

    child.on('close', (code) => {
      resolve(code);
    });

    child.on('error', (err) => {
      reject(err);
    });
  });
}

/**
 * CLI entry point
 */
function main() {
  const args = process.argv.slice(2);
  run(args).then((code) => {
    process.exit(code);
  }).catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

module.exports = { run, main };

// If run directly, act as CLI
if (require.main === module) {
  main();
}
