type ImportMetaGlobOptions = {
  eager?: boolean
  import?: string
  query?: string | Record<string, string | number | boolean>
}

interface ImportMeta {
  glob<T = unknown>(
    pattern: string | string[],
    options?: ImportMetaGlobOptions,
  ): Record<string, T>
}

declare module "*?worker" {
  const WorkerFactory: { new (): Worker }
  export default WorkerFactory
}

declare module "*?worker&url" {
  const url: string
  export default url
}

declare module "*?url" {
  const url: string
  export default url
}

declare module "*.js" {
  const value: unknown
  export default value
}
