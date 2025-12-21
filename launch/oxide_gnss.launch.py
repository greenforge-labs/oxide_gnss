import os
from ament_index_python.packages import get_package_share_directory
from launch import LaunchDescription
from launch.actions import DeclareLaunchArgument
from launch.substitutions import LaunchConfiguration, PathJoinSubstitution
from launch_ros.actions import Node

def generate_launch_description():
    pkg_share = get_package_share_directory('oxide_gnss')
    
    # Arguments
    config_file_arg = DeclareLaunchArgument(
        'config_file',
        default_value=PathJoinSubstitution([pkg_share, 'config', 'default.yaml']),
        description='Path to the configuration YAML file'
    )
    
    log_level_arg = DeclareLaunchArgument(
        'log_level',
        default_value='info',
        description='Logging level (debug, info, warn, error)'
    )

    ntrip_username_arg = DeclareLaunchArgument(
        'ntrip_username',
        default_value=os.environ.get('NTRIP_USERNAME', ''),
        description='NTRIP username (overrides config)'
    )

    ntrip_password_arg = DeclareLaunchArgument(
        'ntrip_password',
        default_value=os.environ.get('NTRIP_PASSWORD', ''),
        description='NTRIP password (overrides config)'
    )

    namespace_arg = DeclareLaunchArgument(
        'namespace',
        default_value='oxide_gnss',
        description='ROS namespace for the node (e.g., gnss_base, gnss_rover)'
    )

    # Node
    node = Node(
        package='oxide_gnss',
        executable='oxide_gnss_node',
        name='gnss_node',
        namespace=LaunchConfiguration('namespace'),
        output='screen',
        parameters=[{
            'use_sim_time': False,
            'config_file': LaunchConfiguration('config_file')
        }],
        # Pass RUST_LOG and NTRIP credentials as environment variables
        additional_env={
            'RUST_LOG': LaunchConfiguration('log_level'),
            'NTRIP_USERNAME': LaunchConfiguration('ntrip_username'),
            'NTRIP_PASSWORD': LaunchConfiguration('ntrip_password')
        }
    )

    return LaunchDescription([
        config_file_arg,
        log_level_arg,
        ntrip_username_arg,
        ntrip_password_arg,
        namespace_arg,
        node
    ])
