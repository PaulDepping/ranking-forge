<script lang="ts">
  import * as Tooltip from "$lib/components/ui/tooltip";
  import Page from "./+page.svelte";
  import type { HeadToHeadEntry, Project } from "$lib/types";

  interface WrapperData {
    user: {
      id: string;
      email: string;
      display_name: string;
      created_at: string;
    } | null;
    project: Project;
    h2h: HeadToHeadEntry[];
    players: { id: string; name: string }[];
    wide: boolean;
  }

  let { data }: { data: WrapperData } = $props();
</script>

<Tooltip.Provider>
  <Page {data} />
</Tooltip.Provider>
