import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

export default mergeConfig(viteConfig, defineConfig({
  // Svelte component tests need the client runtime, including onMount.
  resolve: { conditions: ['browser'] },
}));
