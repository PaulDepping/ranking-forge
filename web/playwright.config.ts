import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: 'tests',
	fullyParallel: true,
	reporter: 'list',
	use: {
		baseURL: 'http://localhost:5174',
		trace: 'on-first-retry'
	},
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		}
	],
	webServer: [
		{
			// Mock API runs first so SvelteKit can reach it at startup
			command: 'node tests/mock-api.js',
			port: 9999,
			reuseExistingServer: true
		},
		{
			command: 'npm run dev -- --port 5174',
			port: 5174,
			reuseExistingServer: !process.env.CI,
			env: {
				INTERNAL_API_URL: 'http://localhost:9999',
				PUBLIC_API_URL: 'http://localhost:9999'
			}
		}
	]
});
