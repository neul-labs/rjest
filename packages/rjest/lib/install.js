const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

const os = require('os');
const { pipeline } = require('stream/promises');

const DOWNLOAD_BASE = process.env.RJEST_DOWNLOAD_URL || 'https://github.com/dipankarsarkar/rjest/releases/download';

function getPlatformInfo() {
  const platform = os.platform();
  const arch = os.arch();

  let osName;
  let archName;

  switch (platform) {
    case 'darwin':
      osName = 'macos';
      break;
    case 'linux':
      osName = 'linux';
      break;
    case 'win32':
      osName = 'windows';
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }

  switch (arch) {
    case 'x64':
    case 'amd64':
      archName = 'x86_64';
      break;
    case 'arm64':
    case 'aarch64':
      archName = 'aarch64';
      break;
    default:
      throw new Error(`Unsupported architecture: ${arch}`);
  }

  return { osName, archName };
}

function getAssetUrl(version, osName, archName) {
  const ext = osName === 'windows' ? '.zip' : '.tar.gz';
  const name = `jest-${osName}-${archName}${ext}`;
  return `${DOWNLOAD_BASE}/v${version}/${name}`;
}

function getChecksumUrl(version, osName, archName) {
  // We'll check checksums from a SHASUMS file
  return `${DOWNLOAD_BASE}/v${version}/SHASUMS256.txt`;
}

async function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    https.get(url, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        downloadFile(response.headers.location, dest).then(resolve).catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        reject(new Error(`Failed to download: ${response.statusCode}`));
        return;
      }

      pipeline(response, file)
        .then(() => resolve())
        .catch(reject);
    }).on('error', reject);
  });
}

async function verifyChecksum(filePath, checksumFile, osName, archName) {
  const expectedChecksum = checksumFile
    .split('\n')
    .find(line => line.includes(`jest-${osName}-${archName}`))
    ?.split(' ')[0];

  if (!expectedChecksum) {
    console.warn('Could not find checksum for binary, skipping verification');
    return;
  }

  const crypto = require('crypto');
  const fileBuffer = fs.readFileSync(filePath);
  const hash = crypto.createHash('sha256').update(fileBuffer).digest('hex');

  if (hash !== expectedChecksum) {
    throw new Error('Checksum verification failed!');
  }

  console.log('Checksum verified successfully');
}

async function extractArchive(archivePath, destDir, osName) {
  const extract = require('extract-zip');

  if (osName === 'windows') {
    await extract(archivePath, { dir: destDir });
  } else {
    const tar = require('tar');
    await tar.extract({
      file: archivePath,
      cwd: destDir
    });
  }
}

async function install() {
  const packageJson = require('../package.json');
  const version = packageJson.version;

  const { osName, archName } = getPlatformInfo();

  console.log(`Installing rjest v${version} for ${osName}-${archName}...`);

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'rjest-'));
  const archivePath = path.join(tempDir, 'archive');
  const extractDir = path.join(tempDir, 'extracted');

  fs.mkdirSync(extractDir);

  try {
    // Download the binary
    const url = getAssetUrl(version, osName, archName);
    console.log(`Downloading from ${url}...`);
    await downloadFile(url, archivePath);

    // Download checksums
    const checksumUrl = getChecksumUrl(version, osName, archName);
    try {
      const checksumData = await new Promise((resolve, reject) => {
        https.get(checksumUrl, (res) => {
          let data = '';
          res.on('data', chunk => data += chunk);
          res.on('end', () => resolve(data));
          res.on('error', reject);
        }).on('error', reject);
      });
      await verifyChecksum(archivePath, checksumData, osName, archName);
    } catch (e) {
      console.warn('Checksum verification failed, continuing anyway:', e.message);
    }

    // Extract the archive
    console.log('Extracting...');
    await extractArchive(archivePath, extractDir, osName);

    // Find the binary
    const files = fs.readdirSync(extractDir);
    let binaryPath = null;

    for (const file of files) {
      const fullPath = path.join(extractDir, file);
      if (fs.statSync(fullPath).isFile() && (file.startsWith('jest') || file.startsWith('rjest'))) {
        binaryPath = fullPath;
        break;
      }
    }

    if (!binaryPath) {
      throw new Error('Could not find binary in archive');
    }

    // Make executable
    fs.chmodSync(binaryPath, 0o755);

    // Move to final location
    const binDir = path.join(__dirname, '..', 'bin');
    fs.mkdirSync(binDir, { recursive: true });

    const binaryName = osName === 'windows' ? 'jest.exe' : 'jest';
    const finalPath = path.join(binDir, binaryName);
    fs.copyFileSync(binaryPath, finalPath);
    fs.chmodSync(finalPath, 0o755);

    // Also create rjest symlink/copy for npx rjest
    const rjestPath = path.join(binDir, osName === 'windows' ? 'rjest.exe' : 'rjest');
    fs.copyFileSync(binaryPath, rjestPath);
    fs.chmodSync(rjestPath, 0o755);

    console.log(`Installed successfully to ${finalPath}`);
  } finally {
    // Cleanup
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

install().catch(err => {
  console.error('Installation failed:', err);
  process.exit(1);
});
