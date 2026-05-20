<script lang="ts">
  import { page } from "$app/state";
  import { Button } from "$lib/components/ui/button";
</script>

{#if page.error?.message === "private_project"}
  <div
    class="flex flex-col items-center justify-center min-h-[60vh] gap-4 text-center"
  >
    <h1 class="text-2xl font-bold">This project is private</h1>
    <p class="text-muted-foreground">
      You need to be a member to view this project.
    </p>
    <Button href="/register">Create an account</Button>
  </div>
{:else}
  <div
    class="flex flex-col items-center justify-center min-h-[60vh] gap-4 text-center"
  >
    <h1 class="text-2xl font-bold">Project not found</h1>
    <p class="text-muted-foreground">
      This project does not exist or has been deleted.
    </p>
    <Button href="/projects">Back to projects</Button>
  </div>
{/if}
