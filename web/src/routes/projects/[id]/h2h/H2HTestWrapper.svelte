<script lang="ts">
  import * as Tooltip from "$lib/components/ui/tooltip";
  import Page from "./+page.svelte";
  import type { HeadToHeadEntry, Player, Project } from "$lib/types";

  interface WrapperData {
    user: {
      id: string;
      email: string;
      display_name: string;
      created_at: string;
    } | null;
    project: Project;
    h2h: HeadToHeadEntry[];
    players: Player[];
    wide: boolean;
  }

  let { data }: { data: WrapperData } = $props();
</script>

<Tooltip.Provider>
  <Page {data} />
</Tooltip.Provider>
