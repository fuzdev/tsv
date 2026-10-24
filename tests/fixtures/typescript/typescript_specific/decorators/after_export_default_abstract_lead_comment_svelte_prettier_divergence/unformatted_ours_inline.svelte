<script lang="ts">
	// Block comment glued to the first decorator of an abstract default-exported class — stays
	// glued to it, as for the class without `abstract`
	export default /* c */ @dec abstract class {}
</script>
