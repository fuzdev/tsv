<script lang="ts">
	// Non-exported `declare`: the `declare` field is emitted before `declarations`
	declare   const   a  :  number ;

	// `export declare`: the `declare` field is emitted after `kind`
	export   declare   const   b  :  string ;
	export   declare   let   c  :  boolean ;
	export   declare   var   d  :  number ;
</script>
