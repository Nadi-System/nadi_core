use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod attrs {
    use crate::prelude::*;
    use abi_stable::std_types::Tuple2;
    use nadi_plugin::{network_func, node_func};

    use string_template_plus::Template;

    /// Set node attributes
    ///
    /// Use this function to set the node attributes of all nodes, or
    /// a select few nodes using the node selection methods (path or
    /// list of nodes)
    ///
    /// # Arguments
    /// - `key=value` - Kwargs of attr = value
    ///
    /// # Error
    /// The function should not error.
    ///
    /// # Example
    /// Following will set the attribute `a2d` to `true` for all nodes
    /// from `A` to `D`
    ///
    /// ```task
    /// node[A -> D] set_attrs(a2d = true)
    /// ```
    #[node_func]
    fn set_attrs(node: &mut NodeInner, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            node.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Set node attributes based on string templates
    ///
    /// # Arguments
    /// - `attr=template` - Kwargs of attr = String template to render
    #[node_func]
    fn set_attrs_render(node: &mut NodeInner, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = node.render(&templ).map_err(|e| e.to_string())?;
            node.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }

    /// Set network attributes
    ///
    /// # Arguments
    /// - `key=value` - Kwargs of attr = value
    #[network_func]
    fn set_attrs(network: &mut Network, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            network.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Set network attributes based on string templates
    ///
    /// # Arguments
    /// - `attr=template` - Kwargs of attr = String template to render
    #[network_func]
    fn set_attrs_render(network: &mut Network, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = network.render(&templ).map_err(|e| e.to_string())?;
            network.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }
}
