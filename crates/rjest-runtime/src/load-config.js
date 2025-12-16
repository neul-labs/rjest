#!/usr/bin/env node
/**
 * Load and normalize Jest configuration for a project.
 * Outputs JSON to stdout for the Rust daemon to parse.
 *
 * Usage: node load-config.js [project-root]
 */

const path = require('path');

async function loadConfig(projectRoot) {
  // Dynamically import jest-config (it may not be installed in all projects)
  let readConfig;
  try {
    const jestConfig = require('jest-config');
    readConfig = jestConfig.readConfig || jestConfig.readConfigs;
  } catch (err) {
    // Fallback: try to load config manually
    return loadConfigManually(projectRoot);
  }

  try {
    const { projectConfig, globalConfig } = await readConfig(
      {}, // argv - empty, we'll use defaults
      projectRoot
    );

    return normalizeConfig(projectConfig, globalConfig, projectRoot);
  } catch (err) {
    // If jest-config fails, try manual loading
    return loadConfigManually(projectRoot);
  }
}

async function loadConfigManually(projectRoot) {
  const fs = require('fs');

  // Try different config file locations
  const configFiles = [
    'jest.config.js',
    'jest.config.ts',
    'jest.config.mjs',
    'jest.config.cjs',
    'jest.config.json',
  ];

  let config = {};

  // Check package.json for jest field
  const pkgPath = path.join(projectRoot, 'package.json');
  if (fs.existsSync(pkgPath)) {
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
      if (pkg.jest) {
        config = pkg.jest;
      }
    } catch (err) {
      // Ignore package.json errors
    }
  }

  // Check for config files
  for (const configFile of configFiles) {
    const configPath = path.join(projectRoot, configFile);
    if (fs.existsSync(configPath)) {
      try {
        if (configFile.endsWith('.json')) {
          config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
        } else if (configFile.endsWith('.ts')) {
          // For TypeScript configs, we need ts-node or similar
          // For now, skip and rely on jest-config
          continue;
        } else {
          // JS config
          delete require.cache[configPath];
          const loaded = require(configPath);
          config = loaded.default || loaded;
        }
        break;
      } catch (err) {
        console.error(`Warning: Failed to load ${configFile}: ${err.message}`);
      }
    }
  }

  return normalizeConfig(config, {}, projectRoot);
}

function normalizeConfig(projectConfig, globalConfig, projectRoot) {
  // Extract the fields we care about
  const normalized = {
    rootDir: projectConfig.rootDir || projectRoot,
    roots: projectConfig.roots || ['<rootDir>'],
    testMatch: projectConfig.testMatch || [
      '**/__tests__/**/*.[jt]s?(x)',
      '**/?(*.)+(spec|test).[jt]s?(x)'
    ],
    testRegex: projectConfig.testRegex || [],
    testPathIgnorePatterns: projectConfig.testPathIgnorePatterns || ['/node_modules/'],
    moduleFileExtensions: projectConfig.moduleFileExtensions || ['js', 'jsx', 'ts', 'tsx', 'json', 'node'],
    moduleNameMapper: projectConfig.moduleNameMapper || {},
    moduleDirectories: projectConfig.moduleDirectories || ['node_modules'],
    modulePaths: projectConfig.modulePaths || [],
    transform: projectConfig.transform || {
      '^.+\\.(t|j)sx?$': 'babel-jest'
    },
    transformIgnorePatterns: projectConfig.transformIgnorePatterns || ['/node_modules/'],
    setupFiles: projectConfig.setupFiles || [],
    setupFilesAfterEnv: projectConfig.setupFilesAfterEnv || [],
    testEnvironment: projectConfig.testEnvironment || 'node',
    testEnvironmentOptions: projectConfig.testEnvironmentOptions || {},
    globals: projectConfig.globals || {},
    collectCoverage: projectConfig.collectCoverage || false,
    collectCoverageFrom: projectConfig.collectCoverageFrom || null,
    coverageDirectory: projectConfig.coverageDirectory || 'coverage',
    coveragePathIgnorePatterns: projectConfig.coveragePathIgnorePatterns || ['/node_modules/'],
    coverageReporters: projectConfig.coverageReporters || ['text', 'lcov'],
    snapshotSerializers: projectConfig.snapshotSerializers || [],
    testTimeout: projectConfig.testTimeout || 5000,
    verbose: projectConfig.verbose || false,
    bail: projectConfig.bail || 0,
    maxWorkers: globalConfig.maxWorkers || '50%',
    projects: projectConfig.projects || null,
    displayName: projectConfig.displayName || null,
    clearMocks: projectConfig.clearMocks || false,
    resetMocks: projectConfig.resetMocks || false,
    restoreMocks: projectConfig.restoreMocks || false,
  };

  // Resolve <rootDir> placeholders
  const resolveRootDir = (value) => {
    if (typeof value === 'string') {
      return value.replace(/<rootDir>/g, normalized.rootDir);
    }
    if (Array.isArray(value)) {
      return value.map(resolveRootDir);
    }
    if (typeof value === 'object' && value !== null) {
      const resolved = {};
      for (const [k, v] of Object.entries(value)) {
        resolved[resolveRootDir(k)] = resolveRootDir(v);
      }
      return resolved;
    }
    return value;
  };

  normalized.roots = resolveRootDir(normalized.roots);
  normalized.setupFiles = resolveRootDir(normalized.setupFiles);
  normalized.setupFilesAfterEnv = resolveRootDir(normalized.setupFilesAfterEnv);
  normalized.moduleNameMapper = resolveRootDir(normalized.moduleNameMapper);

  return normalized;
}

// Main
const projectRoot = process.argv[2] || process.cwd();

loadConfig(path.resolve(projectRoot))
  .then(config => {
    console.log(JSON.stringify(config));
  })
  .catch(err => {
    console.error(JSON.stringify({
      error: true,
      message: err.message,
      stack: err.stack
    }));
    process.exit(1);
  });
