Feature: Plugin Lifecycle
  As a plugin developer
  I want to test plugin loading, initialization, and shutdown
  So that I can ensure my plugin works correctly

  @smoke
  Scenario: Load and initialize a plugin
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized
    When I request the plugin info
    Then the action should succeed
    And the response should be valid JSON

  @smoke
  Scenario: Plugin health check
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized
    When I execute the action "health"
    Then the action should succeed
    And the response should contain "ok"

  Scenario: Plugin configuration
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized
    And the plugin is configured with:
      """
      database.path = "/tmp/test.db"
      log.level = "debug"
      """
    When I execute the action "status"
    Then the action should succeed

  Scenario: Plugin graceful shutdown
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized
    When I shutdown the plugin
    Then all tests should pass
