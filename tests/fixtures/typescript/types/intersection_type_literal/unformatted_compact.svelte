<script lang="ts">
	// Intersection with multiline type literal - & { should hug
type T=A&{
b:B;
};

// In destructuring pattern
const {
rest,
}:OmitStrict<Props, 'content'>&{
text_part:TextPart;
}=$props();

// Multiple properties
type U=Base&{
a:A;
b:B;
c:C;
};
</script>
