<script lang="ts">
	type FnInfer<M> = M extends () => infer U extends string ? U : never;
</script>
