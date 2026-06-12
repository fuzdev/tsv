<script lang="ts">
	type   Inline   =   {  prop  :   GenericType<  A  ,  B  ,  C  ,  DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD  >  }  ;
	type   Expanded   =   {
		prop  :  GenericType<  A  ,  B  ,  C  ,  DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD  >  ;
	}  ;
	type   PropertyInline   =   {
		prop  :  GenericType<  TypeArgument1  ,  TypeArgument2  ,  TypeArgument3  ,  TypeArgument4AAAAAAAAAAAAAAAAAA  >  ;
	}  ;
	type   PropertyWrapped   =   {
		prop  :  GenericType<
			TypeArgument1  ,
			TypeArgument2  ,
			TypeArgument3  ,
			TypeArgument4AAAAAAAAAAAAAAAAAAA
		>  ;
	}  ;
</script>
