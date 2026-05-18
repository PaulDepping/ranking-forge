<script lang="ts">
	import * as Popover from '$lib/components/ui/popover';
	import { RangeCalendar } from '$lib/components/ui/range-calendar';
	import { Button } from '$lib/components/ui/button';
	import type { DateRange } from 'bits-ui';
	import { getLocalTimeZone } from '@internationalized/date';
	import { formatDate } from '$lib/utils';

	let {
		value,
		onSelect,
		placeholder = 'Pick date range',
	}: {
		value: DateRange | undefined;
		onSelect: (range: DateRange | undefined) => void;
		placeholder?: string;
	} = $props();

	let open = $state(false);
	let pending = $state<DateRange | undefined>(undefined);
	$effect(() => {
		if (!open) pending = value;
	});

	function handleValueChange(range: DateRange) {
		pending = range;
		if (range?.start && range?.end) {
			onSelect(range);
			open = false;
		}
	}

	function clearRange() {
		pending = undefined;
		onSelect(undefined);
		open = false;
	}

	const triggerLabel = $derived(
		value?.start && value?.end
			? `${formatDate(value.start.toDate(getLocalTimeZone()))} – ${formatDate(value.end.toDate(getLocalTimeZone()))}`
			: placeholder
	);
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button {...props} variant="outline" class="justify-start font-normal">
				{triggerLabel}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-auto overflow-hidden p-0" align="start">
		<div class="space-y-2 p-3">
			<RangeCalendar
				value={pending}
				captionLayout="dropdown"
				onValueChange={handleValueChange}
			/>
			<div class="flex justify-end px-2 pb-1">
				<Button variant="ghost" size="sm" onclick={clearRange}>Clear</Button>
			</div>
		</div>
	</Popover.Content>
</Popover.Root>
