<script lang="ts">
	type CtorInfer<M> = M extends new () => infer U extends number ? U : never;
</script>
