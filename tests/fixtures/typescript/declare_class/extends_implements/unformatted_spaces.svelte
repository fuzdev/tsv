<script lang="ts">
	declare   class   Single   extends   Base   implements   Contract   {

		prop  :   string  ;
	}

	declare   class   Multiple   extends   Base   implements   Contract  ,   Other   {

		prop
		  :   string  ;
	}

	declare   class   WithGenerics  <  T  >   extends   Base  <  T  >   implements   Contract  <  T  >  ,   Other  <  T  >   {
		value  : 
		  T  ;
	}
</script>
