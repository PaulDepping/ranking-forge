<script lang="ts">
  import * as Tooltip from "$lib/components/ui/tooltip";
  import Page from "./+page.svelte";
  import type { HeadToHeadEntry, Project, Ranking } from "$lib/types";

  interface WrapperData {
    user: {
      id: string;
      email: string;
      display_name: string;
      has_startgg_key: boolean;
      created_at: string;
    } | null;
    project: Project;
    ranking: Ranking;
    rankings: Ranking[];
    h2h: HeadToHeadEntry[];
    players: { id: string; name: string }[];
    wide: boolean;
  }

  let { data }: { data: WrapperData } = $props();
</script>

<Tooltip.Provider>
  <Page {data} />
</Tooltip.Provider>
