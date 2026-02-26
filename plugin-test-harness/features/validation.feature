Feature: Plugin Validation
  As a plugin developer
  I want to validate my plugin meets Skylet requirements
  So that my plugin can be safely loaded and used

  @validation
  Scenario: Plugin has required V2 ABI exports
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    When I request the plugin info
    Then the action should succeed
    And the response should have field "name"
    And the response should have field "version"

  @validation
  Scenario: Plugin returns valid info structure
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    When I request the plugin info
    Then the response should be valid JSON
    And the response field "abi_version" should equal "2"

  @validation
  Scenario: Plugin handles health checks
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    When I execute the action "health"
    Then the action should succeed

  @validation
  Scenario: Plugin handles unknown actions gracefully
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    When I execute the action "definitely_not_a_real_action_12345"
    Then the action should fail
