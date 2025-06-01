/**
 * Basic tests for DataFlare WASM Plugin SDK
 * Note: These are simple Node.js tests without external dependencies
 */

const fs = require('fs');
const path = require('path');

// Simple test framework
function test(name, fn) {
  try {
    fn();
    console.log(`✅ ${name}`);
  } catch (error) {
    console.error(`❌ ${name}: ${error.message}`);
    process.exit(1);
  }
}

function assertEquals(actual, expected, message = '') {
  if (actual !== expected) {
    throw new Error(`Expected ${expected}, got ${actual}. ${message}`);
  }
}

function assertTrue(condition, message = '') {
  if (!condition) {
    throw new Error(`Expected true. ${message}`);
  }
}

// Test that the source files exist and have basic structure
test('Source files exist', () => {
  const srcDir = path.join(__dirname, '..', 'src');
  const files = ['index.ts', 'types.ts', 'plugin.ts', 'runtime.ts', 'utils.ts'];
  
  for (const file of files) {
    const filePath = path.join(srcDir, file);
    assertTrue(fs.existsSync(filePath), `File ${file} should exist`);
    
    const content = fs.readFileSync(filePath, 'utf8');
    assertTrue(content.length > 0, `File ${file} should not be empty`);
  }
});

test('Package.json is valid', () => {
  const packagePath = path.join(__dirname, '..', 'package.json');
  assertTrue(fs.existsSync(packagePath), 'package.json should exist');
  
  const packageContent = fs.readFileSync(packagePath, 'utf8');
  const packageJson = JSON.parse(packageContent);
  
  assertEquals(packageJson.name, '@dataflare/plugin-sdk');
  assertEquals(packageJson.version, '1.0.0');
  assertTrue(packageJson.main.includes('index.js'));
  assertTrue(packageJson.types.includes('index.d.ts'));
});

test('TypeScript config is valid', () => {
  const tsconfigPath = path.join(__dirname, '..', 'tsconfig.json');
  assertTrue(fs.existsSync(tsconfigPath), 'tsconfig.json should exist');
  
  const tsconfigContent = fs.readFileSync(tsconfigPath, 'utf8');
  const tsconfig = JSON.parse(tsconfigContent);
  
  assertEquals(tsconfig.compilerOptions.target, 'ES2020');
  assertEquals(tsconfig.compilerOptions.module, 'ESNext');
  assertTrue(tsconfig.compilerOptions.strict);
});

test('Index file exports are correct', () => {
  const indexPath = path.join(__dirname, '..', 'src', 'index.ts');
  const content = fs.readFileSync(indexPath, 'utf8');
  
  // Check for key exports
  assertTrue(content.includes("export * from './types'"));
  assertTrue(content.includes("export * from './plugin'"));
  assertTrue(content.includes("export * from './runtime'"));
  assertTrue(content.includes("export * from './utils'"));
  assertTrue(content.includes("export const VERSION = '1.0.0'"));
  assertTrue(content.includes("export const SDK_NAME = '@dataflare/plugin-sdk'"));
});

test('Types file has core interfaces', () => {
  const typesPath = path.join(__dirname, '..', 'src', 'types.ts');
  const content = fs.readFileSync(typesPath, 'utf8');
  
  // Check for key interfaces and types
  assertTrue(content.includes('interface DataRecord'));
  assertTrue(content.includes('interface PluginInfo'));
  assertTrue(content.includes('interface PluginCapabilities'));
  assertTrue(content.includes('interface SecurityRequirements'));
  assertTrue(content.includes('enum ComponentType'));
  assertTrue(content.includes('type ProcessingResult'));
  assertTrue(content.includes('interface Logger'));
  assertTrue(content.includes('interface MetricsCollector'));
});

test('Plugin file has base classes', () => {
  const pluginPath = path.join(__dirname, '..', 'src', 'plugin.ts');
  const content = fs.readFileSync(pluginPath, 'utf8');
  
  // Check for key classes
  assertTrue(content.includes('abstract class PluginBase'));
  assertTrue(content.includes('abstract class DataProcessor'));
  assertTrue(content.includes('abstract class DataTransformer'));
  assertTrue(content.includes('abstract class DataSource'));
  assertTrue(content.includes('abstract class DataDestination'));
  assertTrue(content.includes('abstract class DataFilter'));
  assertTrue(content.includes('abstract class DataAggregator'));
  assertTrue(content.includes('abstract class AIProcessor'));
});

test('Runtime file has WASM classes', () => {
  const runtimePath = path.join(__dirname, '..', 'src', 'runtime.ts');
  const content = fs.readFileSync(runtimePath, 'utf8');
  
  // Check for key classes
  assertTrue(content.includes('class WasmRuntime'));
  assertTrue(content.includes('class PluginLoader'));
  assertTrue(content.includes('class WasmPlugin'));
  assertTrue(content.includes('loadModule'));
  assertTrue(content.includes('instantiate'));
  assertTrue(content.includes('processRecord'));
});

test('Utils file has utility functions', () => {
  const utilsPath = path.join(__dirname, '..', 'src', 'utils.ts');
  const content = fs.readFileSync(utilsPath, 'utf8');
  
  // Check for key functions
  assertTrue(content.includes('function validateDataRecord'));
  assertTrue(content.includes('function createDataRecord'));
  assertTrue(content.includes('function serializeResult'));
  assertTrue(content.includes('function deserializeInput'));
  assertTrue(content.includes('function createConsoleLogger'));
  assertTrue(content.includes('function measureTime'));
  assertTrue(content.includes('function retry'));
});

test('README file exists and has content', () => {
  const readmePath = path.join(__dirname, '..', 'README.md');
  assertTrue(fs.existsSync(readmePath), 'README.md should exist');
  
  const content = fs.readFileSync(readmePath, 'utf8');
  assertTrue(content.includes('DataFlare WASM Plugin SDK'));
  assertTrue(content.includes('Installation'));
  assertTrue(content.includes('Quick Start'));
  assertTrue(content.includes('npm install @dataflare/plugin-sdk'));
});

console.log('\n🎉 All tests passed! JavaScript SDK is ready.');
console.log('\nNext steps:');
console.log('1. Run `npm install` to install dependencies');
console.log('2. Run `npm run build` to build the SDK');
console.log('3. Run `npm test` for full test suite');
console.log('4. Publish to npm registry when ready');
