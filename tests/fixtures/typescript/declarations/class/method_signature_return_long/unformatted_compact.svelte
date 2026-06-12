<script lang="ts">
class C {
// 100 chars - stays inline
async send(messageParam: VeryLongParameterTypeNameAAAABB): Promise<VeryLongReturnType | null> {}

// 101 chars - params break, return type stays with )
async send(messageParam: VeryLongParameterTypeNameAAAABBB): Promise<VeryLongReturnType | null> {}
}
</script>
