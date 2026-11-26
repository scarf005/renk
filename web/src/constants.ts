/// <reference path="./vite-env.d.ts" />

type Example = {
  name: string
  slug: string
  source: string
}

const renkExampleModules = import.meta.glob<string>("../../examples/*.rk", {
  eager: true,
  import: "default",
  query: "?raw",
})

const stemFromPath = (path: string) =>
  path.match(/\/([^/.]+)\.[^.]+$/)?.[1] ?? path

const slugFromStem = (stem: string) => stem.replaceAll("_", "-")

const nameFromStem = (stem: string) =>
  stem.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase())

const examplesFromModules = (modules: Record<string, string>) =>
  Object.entries(modules)
    .map(([path, source]) => {
      const stem = stemFromPath(path)
      return { name: nameFromStem(stem), slug: slugFromStem(stem), source }
    })
    .sort((left, right) => left.name.localeCompare(right.name))

export const EXAMPLES = examplesFromModules(renkExampleModules)

export type ExampleName = (typeof EXAMPLES)[number]["name"]

const defaultExample = (examples: Example[], slug: string) =>
  examples.find((example) => example.slug === slug) ?? examples[0]

export const DEFAULT_EXAMPLE_NAME: ExampleName = defaultExample(
  EXAMPLES,
  "hello-user",
).name
export const EXAMPLE_NAMES = EXAMPLES.map(({ name }) => name) as ExampleName[]

const EXAMPLES_BY_NAME = Object.fromEntries(
  EXAMPLES.map((example) => [example.name, example]),
) as Record<ExampleName, (typeof EXAMPLES)[number]>

const EXAMPLE_NAMES_BY_SLUG = Object.fromEntries(
  EXAMPLES.map((example) => [example.slug, example.name]),
) as Record<string, ExampleName>

export const exampleSource = (name: ExampleName) =>
  EXAMPLES_BY_NAME[name].source

export const examplePath = (name: ExampleName) =>
  `/examples/${EXAMPLES_BY_NAME[name].slug}`

export const exampleNameFromSlug = (slug: string) =>
  EXAMPLE_NAMES_BY_SLUG[slug] ?? null

export const exampleNameFromPath = (path: string) => {
  const slug = path.replace(/^#/, "").split("?")[0].match(
    /^\/examples\/([^/]+)$/,
  )
    ?.[1]
  return slug ? exampleNameFromSlug(slug) : null
}
