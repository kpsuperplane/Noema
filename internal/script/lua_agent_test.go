package script

import (
	"reflect"
	"testing"
)

func TestAgentRandomSeedIsLocalAndPreservesIntegerBounds(t *testing.T) {
	const source = `math.randomseed(42, 7)
 local values = {}
 for i=1,8 do values[i] = math.random(-1000,1000) end
 local fraction = math.random()
 assert(fraction >= 0 and fraction < 1)
 assert(math.random(1) == 1)
 assert(math.random(math.mininteger,math.mininteger) == math.mininteger)
 assert(math.random(math.maxinteger,math.maxinteger) == math.maxinteger)
 assert(math.type(math.random(math.mininteger,math.maxinteger)) == "integer")
 assert(math.type(math.random(0)) == "integer")
 math.randomseed()
 return values`
	first, err := Run(source, map[string]any{})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Run(`math.randomseed(99); return math.random()`, map[string]any{}); err != nil {
		t.Fatal(err)
	}
	second, err := Run(source, map[string]any{})
	if err != nil || !reflect.DeepEqual(first, second) {
		t.Fatalf("seed affected another execution: %v, %v, %v", first, second, err)
	}
	for _, invalid := range []string{`math.random(2,1)`, `math.random("invalid")`, `math.randomseed("invalid")`} {
		if _, err := Run("return "+invalid, map[string]any{}); err == nil {
			t.Fatalf("accepted %s", invalid)
		}
	}
}

func TestAdapterProfilesExcludeClockAndRandomness(t *testing.T) {
	for _, profile := range []SandboxProfile{ProfileResponse, ProfileCredential, ProfileRequestAuth} {
		value, err := RunFunctionWithProfile(`return function(input)
   assert(os == nil and math.random == nil and math.randomseed == nil)
   return input
  end`, map[string]any{"ordinary": "café 日本語"}, profile)
		if err != nil || !reflect.DeepEqual(value, map[string]any{"ordinary": "café 日本語"}) {
			t.Fatalf("profile %d changed: %#v, %v", profile, value, err)
		}
	}
}
