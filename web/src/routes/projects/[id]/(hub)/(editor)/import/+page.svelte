<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { Alert } from "$lib/components/ui/alert";
  import * as Card from "$lib/components/ui/card";
  import DateRangePicker from "$lib/components/DateRangePicker.svelte";
  import type { DateRange } from "bits-ui";
  import { makeApi } from "$lib/api";
  import type { Job, ImportProgress } from "$lib/types";
  import { Progress } from "$lib/components/ui/progress";
  import { formatDateTime } from "$lib/utils";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { data, form } = $props();

  let importDialogOpen = $state(false);
  let importFormEl = $state<HTMLFormElement | null>(null);

  let dateRange = $state<DateRange | undefined>(undefined);
  const afterDateStr = $derived(dateRange?.start?.toString() ?? "");
  const beforeDateStr = $derived(dateRange?.end?.toString() ?? "");

  // Local state so we can update after polling; synced when server data changes
  let job = $state<Job | null>(untrack(() => data.job ?? null));
  $effect(() => {
    job = data.job ?? null;
  });

  const isActiveJob = $derived(
    job?.status === "pending" || job?.status === "running",
  );

  const progressLabel = $derived.by((): string => {
    if (!job?.progress) return "";
    const { phase, step, total } = job.progress;
    return phase === "scanning"
      ? `Scanning players (${step} / ${total})`
      : `Importing tournaments (${step} / ${total})`;
  });

  const progressPercent = $derived.by((): number => {
    if (!job?.progress || job.progress.total === 0) return 0;
    return (job.progress.step / job.progress.total) * 100;
  });

  const statusColors: Record<
    string,
    "default" | "secondary" | "destructive" | "outline"
  > = {
    pending: "secondary",
    running: "default",
    done: "outline",
    failed: "destructive",
  };

  $effect(() => {
    if (!isActiveJob) return;
    const interval = setInterval(async () => {
      const api = makeApi(fetch);
      const res = await api.get(`/projects/${data.project.id}/import`);
      if (res.ok) {
        job = (await res.json()) as Job;
      }
    }, 1000);
    return () => clearInterval(interval);
  });
</script>

<div class="space-y-6 max-w-lg">
  <h2 class="text-lg font-semibold">Import tournaments</h2>
  <p class="text-sm text-muted-foreground">
    Fetches all start.gg tournaments for your players and imports them. This may
    take a minute.
  </p>

  {#if !data.project.owner_has_startgg_key}
    <Card.Root>
      <Card.Content class="p-4 space-y-2">
        {#if data.project.user_role === "owner"}
          <p class="text-sm font-medium">
            A start.gg API key is required to run imports.
          </p>
          <p class="text-sm text-muted-foreground">
            Add your key in
            <a href="/account" class="underline">account settings</a>, or get
            one at
            <a
              href="https://start.gg/admin/profile/developer"
              target="_blank"
              rel="noopener noreferrer"
              class="underline">start.gg/admin/profile/developer</a
            >.
          </p>
        {:else}
          <p class="text-sm text-muted-foreground">
            The project owner needs to configure a start.gg API key before
            imports can run.
          </p>
        {/if}
      </Card.Content>
    </Card.Root>
  {:else}
    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    {#if job}
      <Card.Root class="py-0">
        <Card.Content class="p-4 space-y-2">
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium">Status:</span>
            <Badge variant={statusColors[job.status]}>{job.status}</Badge>
            {#if isActiveJob}
              <span class="text-xs text-muted-foreground animate-pulse"
                >updating…</span
              >
            {/if}
          </div>
          {#if job.status === "pending"}
            <p class="text-sm text-muted-foreground">Waiting to start…</p>
          {/if}
          {#if job.status === "running" && job.progress}
            <div class="space-y-1">
              <p class="text-sm text-muted-foreground">{progressLabel}</p>
              <Progress value={progressPercent} class="h-2" />
            </div>
          {/if}
          {#if job.error}
            <p class="text-sm text-destructive">{job.error}</p>
          {/if}
          <p class="text-xs text-muted-foreground">
            Started {formatDateTime(job.created_at)}
          </p>
          {#if job.status === "failed"}
            <form
              method="POST"
              use:enhance={() => {
                return ({ result }) => {
                  if (result.type === "success" && result.data?.job) {
                    job = result.data.job as Job;
                  }
                };
              }}
            >
              <input
                type="hidden"
                name="after_date"
                value={job.after_date ?? ""}
              />
              <input
                type="hidden"
                name="before_date"
                value={job.before_date ?? ""}
              />
              <Button type="submit" variant="outline" size="sm">Retry</Button>
            </form>
          {/if}
        </Card.Content>
      </Card.Root>
    {/if}

    <form
      method="POST"
      class="space-y-4"
      bind:this={importFormEl}
      use:enhance={() => {
        return ({ result }) => {
          if (result.type === "success" && result.data?.job) {
            job = result.data.job as Job;
          }
        };
      }}
    >
      <input type="hidden" name="after_date" value={afterDateStr} />
      <input type="hidden" name="before_date" value={beforeDateStr} />
      <DateRangePicker
        value={dateRange}
        onSelect={(r) => {
          dateRange = r;
        }}
        placeholder="All time"
      />
      <p class="text-xs text-muted-foreground">
        Leave blank to import all tournaments.
      </p>
      <Button
        type="button"
        onclick={() => {
          if (isActiveJob) {
            importDialogOpen = true;
          } else {
            importFormEl?.requestSubmit();
          }
        }}
      >
        {job ? "Re-import" : "Start import"}
      </Button>
    </form>
  {/if}
</div>

<AlertDialog.Root bind:open={importDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Import already running</AlertDialog.Title>
      <AlertDialog.Description>
        An import is currently in progress. Start a new one anyway?
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        onclick={() => {
          importDialogOpen = false;
          importFormEl?.requestSubmit();
        }}
      >
        Start import
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
