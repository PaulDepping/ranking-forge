import { render, screen, fireEvent } from '@testing-library/svelte';
import { vi, describe, it, expect, beforeEach } from 'vitest';

vi.mock('mode-watcher', () => ({
	toggleMode: vi.fn(),
	ModeWatcher: {},
	mode: { current: 'light' as 'light' | 'dark' }
}));

import { toggleMode } from 'mode-watcher';
import ThemeToggle from './ThemeToggle.svelte';

describe('ThemeToggle', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('renders a button with aria-label "Toggle theme"', () => {
		render(ThemeToggle);
		expect(screen.getByRole('button', { name: 'Toggle theme' })).toBeInTheDocument();
	});

	it('calls toggleMode when clicked', () => {
		render(ThemeToggle);
		fireEvent.click(screen.getByRole('button', { name: 'Toggle theme' }));
		expect(toggleMode).toHaveBeenCalledOnce();
	});
});
