import { execSync } from 'child_process'

export const versions = ['rmk-v0.7.8']

versions.forEach((version) => {
  const command = `git archive origin/${version} docs/docs/main | tar -x -C docs/docs/ --transform "s|docs/docs/main|${version}|"`
  execSync(command, { stdio: 'inherit', shell: 'bash' })
})
