import { execSync } from 'child_process'
import { readFileSync } from 'fs'

const versions = JSON.parse(readFileSync(new URL('./versions.json', import.meta.url)))

versions.forEach((branch) => {
  const version = branch.split('/').pop()
  const command = `git archive ${branch} docs/main | tar -x -C docs/ --transform "s|docs/main|${version}|"`
  console.log(`Fetched docs ${branch}`)
  execSync(command, { stdio: 'inherit', shell: 'bash' })
})
