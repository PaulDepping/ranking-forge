<script lang="ts">
  import { enhance } from "$app/forms";
  import { invalidateAll } from "$app/navigation";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";

  let {
    playerId,
    accountId,
    displayName,
    handle,
  }: {
    playerId: string;
    accountId: string;
    displayName: string | null;
    handle: string;
  } = $props();
</script>

<form
  method="POST"
  action="?/unlinkAccount"
  use:enhance={() => {
    return async ({ result, update }) => {
      if (result.type === "success") {
        await invalidateAll();
      } else {
        await update();
      }
    };
  }}
  class="inline-flex"
>
  <input type="hidden" name="pid" value={playerId} />
  <input type="hidden" name="aid" value={accountId} />
  <Badge variant="secondary" class="gap-1 pr-1">
    {displayName ?? handle}
    <Button
      type="submit"
      variant="ghost"
      size="icon"
      class="ml-0.5 h-4 w-4 rounded-full p-0"
      title="Remove">×</Button
    >
  </Badge>
</form>
