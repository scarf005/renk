// Rusk has Rust's tokens plus `then`, so its grammar extends the bundled Rust one. mdBook
// highlights before this script runs and leaves blocks in unknown languages untouched.
hljs.registerLanguage("rsk", (hljs) => {
  const rust = hljs.getLanguage("rust").rawDefinition(hljs)
  return {
    ...rust,
    name: "Rusk",
    aliases: ["rusk"],
    keywords: { ...rust.keywords, keyword: `${rust.keywords.keyword} then` },
  }
})
document.querySelectorAll("code.language-rsk").forEach((block) => hljs.highlightBlock(block))
