<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import {
    Card,
    CardHeader,
    CardTitle,
    CardDescription,
    CardFooter,
  } from "$lib/components/ui/card";
  import * as Empty from "$lib/components/ui/empty";
  import { formatDate } from "$lib/utils";

  let { data } = $props();
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Projects</h1>
    <Button href="/projects/new">New project</Button>
  </div>

  {#if data.projects.length === 0}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>No projects yet</Empty.Title>
        <Empty.Description
          >Create a project to start building a power ranking.</Empty.Description
        >
      </Empty.Header>
    </Empty.Root>
  {:else}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each data.projects as project (project.id)}
        <Card>
          <CardHeader>
            <CardTitle>
              <a href="/projects/{project.id}/players" class="hover:underline"
                >{project.name}</a
              >
            </CardTitle>
            {#if project.game_name}
              <CardDescription>{project.game_name}</CardDescription>
            {/if}
          </CardHeader>
          <CardFooter>
            <span class="text-xs text-muted-foreground">
              {formatDate(project.created_at)}
            </span>
          </CardFooter>
        </Card>
      {/each}
    </div>
  {/if}
</div>
