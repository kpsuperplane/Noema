import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const dir = path.dirname(fileURLToPath(import.meta.url));
const web = path.resolve(dir, '../../../apps/web');
const require = createRequire(path.join(web, 'package.json'));
const {build} = await import(require.resolve('vite'));
const {default: react} = await import(require.resolve('@vitejs/plugin-react'));
// Escape template strings so vendor snippet whitespace survives Git whitespace checks.
await build({configFile:false, esbuild:{supported:{'template-literal':false}}, root:path.join(dir,'source'), base:'./', publicDir:false,
 plugins:[react({babel:{plugins:[[require.resolve('@stylexjs/babel-plugin'),{dev:false,debug:false,runtimeInjection:true,treeshakeCompensation:true,unstable_moduleResolution:{type:'commonJS',rootDir:web}}]]}})],
 resolve:{alias:[{find:'@',replacement:path.join(web,'src')},{find:/^(@astryxdesign\/core|@astryxdesign\/theme-neutral|react-dom|react|lucide-react|@stylexjs\/stylex)(\/.*)?$/,replacement:'$1$2',customResolver:(id)=>require.resolve(id)}]},
 build:{outDir:path.join(dir,'gallery'),emptyOutDir:true,chunkSizeWarningLimit:1500}});
