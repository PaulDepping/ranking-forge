import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
	plugins: [svelte()],
	resolve: {
		alias: {
			$lib: resolve('./src/lib'),
			'$env/dynamic/public': resolve('./src/__mocks__/env.ts'),
			'$env/dynamic/private': resolve('./src/__mocks__/env.private.ts'),
		},
		conditions: ['browser']
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}'],
		globals: true,
		environment: 'jsdom',
		setupFiles: ['src/setupTests.ts']
	}
});
